/**
 * Store Pinia du module Tunnels.
 *
 * Deux responsabilités, comme le store Communes :
 * 1. **état de vue** de l'onglet Tunnels (`file`, brouillon de bornes,
 *    sélection, sauvegarde, validation) ;
 * 2. **file d'attente globale** de détection via le **serveur hermes** : une
 *    trace à la fois, contrôle des artefacts (`ensureReady`), puis
 *    `tunnels_detect_hermes`.
 *
 * La file est déclenchée après un import propre, après une validation d'audit,
 * au lancement sur les traces déjà propres, au retour du service hermes et par
 * le bouton « Relancer » de l'onglet. Elle ne montre aucune confirmation : le
 * bilan est regroupé en une notification par catégorie (succès, hors couverture,
 * échecs) et l'état final vit dans le registre (`tunnels_status`).
 *
 * Le moteur local (PBF de la clé USB) reste exposé — `support`,
 * `requirements`, `progress`, `refreshSupport`, `refreshRequirements`,
 * `bindProgress` — mais n'est plus appelé par le flux nominal.
 */

import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

import { parseHermesError } from '../utils/hermes'
import {
  applyTunnelBounds,
  constrainTunnelBound,
  groupTunnels,
  MIN_TUNNEL_LENGTH_KM,
  tunnelRangeOverlaps,
} from '../utils/altimetrieTunnels'
import { useHermesStore } from './hermes'
import { useTracesStore } from './traces'
import { useUiStore } from './ui'

// ─── Types (miroir des structs Rust, camelCase) ───────────────────────

export interface TunnelParams {
  distanceToleranceM: number
  angleToleranceDeg: number
  bboxMarginM: number
  bearingWindowM: number
  minRunLength: number
  /** Longueur minimale d'un tunnel détecté (m). */
  minLengthM: number
}

export interface PbfFileRef {
  name: string
  modifiedAt?: string | null
}

export interface Tunnel {
  id: string
  wayId?: string | null
  /** Ways OSM couvertes par le tunnel logique (détection hermes). */
  wayIds?: string[] | null
  name?: string | null
  highway?: string | null
  tunnelType?: string | null
  startKm: number
  endKm: number
  lengthM: number
  detectionMethod: 'osm' | 'manual' | string
  maxDistanceM?: number | null
  maxAngleDiffDeg?: number | null
  groupId?: string | null
}

export interface TunnelFile {
  version: number
  traceId: string
  detectedAt?: string | null
  updatedAt: string
  pbfFiles: PbfFileRef[]
  params: TunnelParams
  tunnels: Tunnel[]
}

export interface TunnelGroup {
  /** Identifiant de sélection : celui du premier tronçon. */
  id: string
  /** Groupe de fusion commun, ou `null` pour un tunnel isolé. */
  groupId: string | null
  tunnels: Tunnel[]
  startKm: number
  endKm: number
  /** Somme des longueurs des tronçons, jamais l'étendue avec l'écart. */
  lengthM: number
  merged: boolean
}

export interface TunnelBoundsDraft {
  tunnelId: string
  startKm: number
  endKm: number
}

export interface TunnelRequirementRegion {
  id: string
  name: string
  pbfFilename: string
  sizeMb?: number | null
  present: boolean
  /** PBF manquant au-dessus du plafond configuré. */
  tooLarge: boolean
}

export interface TunnelRequirements {
  readiness: string
  folder: string | null
  regions: TunnelRequirementRegion[]
  missingCount: number
  message?: string | null
}

export interface TunnelSupportStatus {
  active: boolean
  configured: boolean
  folder: string | null
  readable: boolean
  regionsPresent: boolean
  regionsVersion: string | null
  pbfCount: number
}

export interface TunnelProgress {
  traceId: string
  stage: 'regions' | 'pbf' | 'detect' | string
  region?: string | null
  filename?: string | null
  bytesDone: number
  bytesTotal: number
  percent: number
}

export interface TunnelPrepareResult {
  file: TunnelFile
  status: string
}

/**
 * Retour de `tunnels_detect_hermes` : le fichier écrit, le statut posé au
 * registre, le dataset servi et le verdict de couverture.
 */
export interface TunnelDetection {
  file: TunnelFile
  /** `waiting_validate`, `success` ou `outside_coverage`. */
  status: string
  datasetId: string
  outsideCoverage: boolean
}

/** Origine d'un lot : un clic parle, le rattrapage du lancement se tait. */
type RunReason = 'request' | 'catch-up'

// ─── Constantes ───────────────────────────────────────────────────────

const DEFAULT_TUNNEL_PARAMS: TunnelParams = {
  distanceToleranceM: 20,
  angleToleranceDeg: 25,
  bboxMarginM: 500,
  bearingWindowM: 30,
  minRunLength: 2,
  minLengthM: 25,
}

const DEFAULT_SUPPORT: TunnelSupportStatus = {
  active: true,
  configured: false,
  folder: null,
  readable: false,
  regionsPresent: false,
  regionsVersion: null,
  pbfCount: 0,
}

let tunnelIdCounter = 0
let groupIdCounter = 0

function nextTunnelId(): string {
  tunnelIdCounter += 1
  return `m-${Date.now().toString(36)}-${tunnelIdCounter}`
}

function nextGroupId(): string {
  groupIdCounter += 1
  return `g-${Date.now().toString(36)}-${groupIdCounter}`
}

function round6(value: number): number {
  return Math.round(value * 1e6) / 1e6
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function emptyFile(traceId: string): TunnelFile {
  return {
    version: 1,
    traceId,
    detectedAt: null,
    updatedAt: '',
    pbfFiles: [],
    params: { ...DEFAULT_TUNNEL_PARAMS },
    tunnels: [],
  }
}

/** Statuts qui closent la détection automatique (relance manuelle sinon). */
function isTerminalStatus(status: string | null | undefined): boolean {
  return (
    status === 'waiting_validate' ||
    status === 'success' ||
    status === 'outside_coverage'
  )
}

export const useTunnelsStore = defineStore('tunnels', () => {
  const ui = useUiStore()
  const hermes = useHermesStore()

  // ─── État de vue ───────────────────────────────────────────────────
  const file = ref<TunnelFile | null>(null)
  const fileExists = ref(false)
  const loading = ref(false)
  const saving = ref(false)
  const validated = ref(false)
  const dirty = ref(false)
  const selectedTunnelId = ref<string | null>(null)
  const boundsDraft = ref<TunnelBoundsDraft | null>(null)
  const viewTraceId = ref<string | null>(null)
  const requirements = ref<TunnelRequirements | null>(null)
  const loadError = ref<string | null>(null)

  // ─── File d'attente et support ─────────────────────────────────────
  const queue = ref<string[]>([])
  const currentTraceId = ref<string | null>(null)
  const isRunning = ref(false)
  const progress = ref<TunnelProgress | null>(null)
  const support = ref<TunnelSupportStatus>({ ...DEFAULT_SUPPORT })
  const supportLoaded = ref(false)
  const lastError = ref<string | null>(null)
  const lastErrorTraceId = ref<string | null>(null)
  /**
   * Traces dont la dernière détection est sortie de la couverture du dataset
   * servi, avec le dataset en question. État de **session** : le fichier ne
   * conserve que le statut `outside_coverage`, pas le nom du dataset.
   */
  const outsideCoverage = ref<Record<string, string>>({})

  /** Traces dont la détection a échoué dans cette session. */
  const failed = new Set<string>()
  /** Demandes explicites : elles ne sont pas écartées par l'état du registre. */
  const forced = new Set<string>()
  let progressBound = false

  // ─── Getters ───────────────────────────────────────────────────────

  const displayFile = computed<TunnelFile | null>(() => {
    const current = file.value
    const draft = boundsDraft.value
    if (!current || !draft) return current
    return {
      ...current,
      tunnels: current.tunnels.map(tunnel =>
        tunnel.id === draft.tunnelId
          ? applyTunnelBounds(tunnel, draft.startKm, draft.endKm)
          : tunnel,
      ),
    }
  })

  const tunnels = computed<Tunnel[]>(() =>
    [...(displayFile.value?.tunnels ?? [])].sort((a, b) => a.startKm - b.startKm),
  )

  const groups = computed<TunnelGroup[]>(() => groupTunnels(tunnels.value))

  const tunnelCount = computed(() => groups.value.length)

  const selectedGroup = computed<TunnelGroup | null>(
    () =>
      groups.value.find(group =>
        group.tunnels.some(tunnel => tunnel.id === selectedTunnelId.value),
      ) ?? null,
  )

  const selectedTunnelIds = computed<string[]>(
    () => selectedGroup.value?.tunnels.map(tunnel => tunnel.id) ?? [],
  )

  function isPending(traceId: string): boolean {
    return currentTraceId.value === traceId || queue.value.includes(traceId)
  }

  function isPreparing(traceId: string): boolean {
    return currentTraceId.value === traceId
  }

  function progressFor(traceId: string): TunnelProgress | null {
    return progress.value?.traceId === traceId ? progress.value : null
  }

  /** Dataset dont la trace est sortie à la dernière détection, ou `null`. */
  function outsideCoverageDataset(traceId: string): string | null {
    return outsideCoverage.value[traceId] ?? null
  }

  // ─── Chargement et support ─────────────────────────────────────────

  async function loadOrCreate(
    traceId: string,
    persistedValidated = false,
    outsideCoverageFile = false,
  ): Promise<void> {
    viewTraceId.value = traceId
    loading.value = true
    loadError.value = null
    try {
      const loaded = await invoke<TunnelFile | null>('tunnels_load', { traceId })
      if (viewTraceId.value !== traceId) return
      if (loaded) {
        file.value = loaded
        fileExists.value = true
        // Un fichier vide hors couverture n'est pas une analyse validée : le
        // statut du registre porte l'information, la vue ne doit pas afficher
        // « validé ».
        validated.value =
          persistedValidated ||
          (!outsideCoverageFile && loaded.tunnels.length === 0)
      } else {
        file.value = emptyFile(traceId)
        fileExists.value = false
        validated.value = false
      }
      dirty.value = false
      selectedTunnelId.value = null
      boundsDraft.value = null
    } catch (error) {
      if (viewTraceId.value !== traceId) return
      file.value = emptyFile(traceId)
      fileExists.value = false
      validated.value = false
      dirty.value = false
      selectedTunnelId.value = null
      boundsDraft.value = null
      loadError.value = errorMessage(error)
      console.error(`[tunnels] Chargement impossible pour ${traceId} :`, error)
    } finally {
      loading.value = false
    }
  }

  async function refreshSupport(): Promise<void> {
    try {
      support.value = await invoke<TunnelSupportStatus>('tunnels_support_status')
    } catch (error) {
      console.error('[tunnels] État du support OSM illisible :', error)
      support.value = { ...DEFAULT_SUPPORT }
    } finally {
      supportLoaded.value = true
    }
  }

  async function refreshRequirements(traceId: string): Promise<void> {
    try {
      const result = await invoke<TunnelRequirements>('tunnels_requirements', {
        traceId,
      })
      if (viewTraceId.value === traceId) requirements.value = result
    } catch (error) {
      console.error(`[tunnels] Ressources OSM illisibles pour ${traceId} :`, error)
      if (viewTraceId.value === traceId) requirements.value = null
    }
  }

  // ─── Édition locale ────────────────────────────────────────────────

  function selectTunnel(tunnelId: string | null): void {
    if (tunnelId === null) {
      selectedTunnelId.value = null
      return
    }
    const exists = file.value?.tunnels.some(tunnel => tunnel.id === tunnelId)
    selectedTunnelId.value = exists ? tunnelId : null
  }

  function createTunnel(startKm: number, endKm: number): Tunnel {
    if (!file.value) {
      throw new Error("Aucun fichier de tunnels n'est chargé.")
    }
    const start = round6(startKm)
    const end = round6(endKm)
    if (end - start < MIN_TUNNEL_LENGTH_KM) {
      throw new Error('Le tunnel doit mesurer au moins 10 m.')
    }
    if (tunnelRangeOverlaps(file.value.tunnels, start, end)) {
      throw new Error('Le tunnel chevauche un tunnel existant.')
    }

    const tunnel: Tunnel = {
      id: nextTunnelId(),
      startKm: start,
      endKm: end,
      lengthM: Math.round((end - start) * 1000 * 10) / 10,
      detectionMethod: 'manual',
      groupId: null,
    }
    file.value = {
      ...file.value,
      tunnels: [...file.value.tunnels, tunnel],
    }
    dirty.value = true
    selectedTunnelId.value = tunnel.id
    return tunnel
  }

  function draftTunnelBound(
    tunnelId: string,
    handle: 'entrance' | 'exit',
    km: number,
    totalKm: number,
  ): void {
    const current = file.value
    if (!current) return
    const tunnel = current.tunnels.find(candidate => candidate.id === tunnelId)
    if (!tunnel || tunnel.groupId) return
    const bounds = constrainTunnelBound(
      tunnel,
      handle,
      km,
      current.tunnels,
      totalKm,
    )
    boundsDraft.value = { tunnelId, ...bounds }
  }

  function clearTunnelDraft(): void {
    boundsDraft.value = null
  }

  function updateTunnelBound(
    tunnelId: string,
    handle: 'entrance' | 'exit',
    km: number,
    totalKm: number,
  ): void {
    const current = file.value
    if (!current) return
    const tunnel = current.tunnels.find(candidate => candidate.id === tunnelId)
    if (!tunnel || tunnel.groupId) return

    const bounds = constrainTunnelBound(
      tunnel,
      handle,
      km,
      current.tunnels,
      totalKm,
    )
    if (
      Math.abs(bounds.startKm - tunnel.startKm) < 1e-4 &&
      Math.abs(bounds.endKm - tunnel.endKm) < 1e-4
    ) {
      boundsDraft.value = null
      return
    }

    file.value = {
      ...current,
      tunnels: current.tunnels.map(candidate =>
        candidate.id === tunnelId
          ? applyTunnelBounds(candidate, bounds.startKm, bounds.endKm)
          : candidate,
      ),
    }
    boundsDraft.value = null
    dirty.value = true
  }

  function deleteGroup(groupId: string): void {
    const current = file.value
    const group = groups.value.find(candidate => candidate.id === groupId)
    if (!current || !group) return
    const ids = new Set(group.tunnels.map(tunnel => tunnel.id))
    file.value = {
      ...current,
      tunnels: current.tunnels.filter(tunnel => !ids.has(tunnel.id)),
    }
    if (selectedTunnelId.value && ids.has(selectedTunnelId.value)) {
      selectedTunnelId.value = null
    }
    dirty.value = true
  }

  function mergeWithPrevious(groupId: string, maxGapM: number): boolean {
    const current = file.value
    if (!current) return false
    const ordered = groups.value
    const index = ordered.findIndex(group => group.id === groupId)
    if (index <= 0) return false

    const target = ordered[index]
    const previous = ordered[index - 1]
    const gapM = (target.startKm - previous.endKm) * 1000
    if (gapM < -1e-6 || gapM > maxGapM) return false

    const targetGroupId = previous.groupId ?? nextGroupId()
    const ids = new Set(
      [...previous.tunnels, ...target.tunnels].map(tunnel => tunnel.id),
    )
    file.value = {
      ...current,
      tunnels: current.tunnels.map(tunnel =>
        ids.has(tunnel.id) ? { ...tunnel, groupId: targetGroupId } : tunnel,
      ),
    }
    selectedTunnelId.value = previous.tunnels[0].id
    dirty.value = true
    return true
  }

  function splitGroup(groupId: string): boolean {
    const current = file.value
    const group = groups.value.find(candidate => candidate.id === groupId)
    if (!current || !group || !group.merged) return false

    const ids = new Set(group.tunnels.map(tunnel => tunnel.id))
    file.value = {
      ...current,
      tunnels: current.tunnels.map(tunnel =>
        ids.has(tunnel.id) ? { ...tunnel, groupId: null } : tunnel,
      ),
    }
    selectedTunnelId.value = group.tunnels[0].id
    dirty.value = true
    return true
  }

  async function save(traceId: string): Promise<TunnelFile> {
    const current = file.value
    if (!current) {
      throw new Error("Aucun fichier de tunnels n'est chargé.")
    }
    saving.value = true
    try {
      const saved = await invoke<TunnelFile>('tunnels_save', {
        traceId,
        tunnels: current.tunnels,
      })
      file.value = saved
      fileExists.value = true
      validated.value = true
      dirty.value = false
      boundsDraft.value = null
      await useTracesStore().loadTraces()
      return saved
    } finally {
      saving.value = false
    }
  }

  function reset(): void {
    file.value = null
    fileExists.value = false
    loading.value = false
    saving.value = false
    validated.value = false
    dirty.value = false
    selectedTunnelId.value = null
    boundsDraft.value = null
    viewTraceId.value = null
    requirements.value = null
    loadError.value = null
    lastError.value = null
    lastErrorTraceId.value = null
  }

  // ─── File d'attente ────────────────────────────────────────────────

  /** Détecte les tunnels d'une trace via hermes et retient la couverture. */
  async function detect(traceId: string): Promise<TunnelDetection> {
    const detection = await invoke<TunnelDetection>('tunnels_detect_hermes', {
      traceId,
    })
    if (detection.outsideCoverage) {
      outsideCoverage.value[traceId] = detection.datasetId
    } else {
      delete outsideCoverage.value[traceId]
    }
    return detection
  }

  /**
   * La fonctionnalité est-elle activée ? `Tunnels.Support.actif` reste le
   * commutateur du module, même si le moteur est désormais hermes.
   */
  async function featureEnabled(reason: RunReason): Promise<boolean> {
    if (!supportLoaded.value) await refreshSupport()
    if (support.value.active) return true
    if (reason === 'request') {
      ui.showWarning('La détection des tunnels est désactivée dans les paramètres.')
    }
    return false
  }

  /**
   * Le service est-il prêt pour le lot ? Une seule vérification par lot : le
   * dataset servi et les artefacts `tunnels-logiques.json` + `mm/` doivent être
   * `ready`. Le backend reste seul juge (il relit `/serving`).
   */
  async function readyToDetect(reason: RunReason): Promise<boolean> {
    const readiness = await hermes.ensureReady('tunnels')
    if (readiness.ok) return true

    // Au lancement, le serveur est en veille : l'échec est attendu, et le chip
    // Hermes en porte déjà l'explication. Seules les causes **durables**
    // (dataset absent, artefacts non préparés) méritent une notification.
    const durable =
      readiness.code !== 'HERMES_OFFLINE' && readiness.code !== 'HERMES_TIMEOUT'
    if (reason === 'request' || durable) {
      ui.showWarning(
        readiness.message ?? 'Détection des tunnels indisponible pour le moment.',
      )
    }
    return false
  }

  /**
   * Tâche de fond : vide la file, une trace à la fois.
   *
   * La préparation du serveur est vérifiée **une fois par lot** ; l'éligibilité,
   * elle, est revérifiée **avant chaque trace**, registre relu à la source.
   */
  async function runQueue(reason: RunReason): Promise<void> {
    if (isRunning.value) return
    isRunning.value = true
    const tracesStore = useTracesStore()

    const failures: string[] = []
    const uncovered: string[] = []
    let done = 0

    try {
      if (!(await featureEnabled(reason))) {
        queue.value = []
        return
      }

      let ready = false
      try {
        ready = await readyToDetect(reason)
      } catch (error) {
        console.error('[tunnels] Préparation hermes impossible :', error)
      }
      if (!ready) {
        queue.value = []
        return
      }

      while (queue.value.length > 0) {
        const traceId = queue.value.shift()
        if (!traceId || failed.has(traceId)) {
          forced.delete(traceId ?? '')
          continue
        }

        currentTraceId.value = traceId
        progress.value = null
        lastError.value = null
        lastErrorTraceId.value = null

        await tracesStore.loadTraces()
        const trace = tracesStore.traces.find(candidate => candidate.id === traceId)
        const isForced = forced.has(traceId)
        forced.delete(traceId)

        if (!trace || trace.audit_status !== 'clean') continue
        if (!isForced && isTerminalStatus(trace.tunnels_status)) continue

        try {
          const result = await detect(traceId)
          failed.delete(traceId)
          done += 1
          if (result.outsideCoverage) uncovered.push(trace.name)
          if (viewTraceId.value === traceId) {
            file.value = result.file
            fileExists.value = true
            validated.value = result.status === 'success'
            dirty.value = false
            selectedTunnelId.value = null
            boundsDraft.value = null
          }
          await tracesStore.loadTraces()
        } catch (error) {
          // Les commandes hermes préfixent leurs erreurs par un code machine :
          // le message utilisateur est nettoyé, le code reste dans la console.
          const parsed = parseHermesError(error)
          failed.add(traceId)
          lastError.value = parsed.message
          lastErrorTraceId.value = traceId
          failures.push(`${trace.name} (${parsed.message})`)
          console.warn(
            `[tunnels] Détection impossible pour « ${trace.name} » [${parsed.code}] : ${parsed.message}`,
          )
          await tracesStore.loadTraces()
        } finally {
          progress.value = null
        }
      }
    } finally {
      isRunning.value = false
      currentTraceId.value = null
      progress.value = null
      report(done, failures, uncovered)
    }
  }

  /**
   * Bilan de fin de tâche : **une seule** notification par catégorie, jamais une
   * par circuit — un rattrapage au lancement peut en enchaîner des dizaines.
   */
  function report(done: number, failures: string[], uncovered: string[]): void {
    const detected = done - uncovered.length
    if (detected > 0) {
      ui.showSuccess(
        detected === 1
          ? 'Tunnels détectés pour 1 circuit.'
          : `Tunnels détectés pour ${detected} circuits.`,
      )
    }
    if (uncovered.length > 0) {
      const first =
        uncovered.length > 1
          ? `${uncovered[0]} +${uncovered.length - 1}`
          : uncovered[0]
      ui.showWarning(
        `Aucun tunnel détecté pour ${first} : la trace sort de la couverture du dataset servi.`,
      )
    }
    if (failures.length === 0) return
    const first =
      failures.length > 1 ? `${failures[0]} +${failures.length - 1}` : failures[0]
    ui.showWarning(
      `Détection des tunnels incomplète : ${detected} réussite(s), échec sur ${first}.`,
    )
  }

  /**
   * Demande explicite : relance même si le registre indique déjà une analyse.
   * La trace passe en tête de file.
   */
  function request(traceId: string): void {
    if (currentTraceId.value === traceId) return
    failed.delete(traceId)
    forced.add(traceId)
    if (queue.value.includes(traceId)) return
    queue.value.unshift(traceId)
    void runQueue('request')
  }

  /**
   * Rattrapage au lancement : traces propres sans analyse terminale. Rien à
   * faire → aucune tâche ne démarre. Le module désactivé ne démarre rien.
   * Appelée aussi au retour du service hermes.
   */
  async function startCatchUp(): Promise<void> {
    await refreshSupport()
    if (!support.value.active) return

    const tracesStore = useTracesStore()
    await tracesStore.loadTraces()
    const pending = tracesStore.traces
      .filter(
        trace =>
          trace.audit_status === 'clean' &&
          !isTerminalStatus(trace.tunnels_status) &&
          !failed.has(trace.id),
      )
      .map(trace => trace.id)

    if (pending.length === 0) return
    for (const traceId of pending) {
      if (!isPending(traceId)) queue.value.push(traceId)
    }
    void runQueue('catch-up')
  }

  async function bindProgress(): Promise<void> {
    if (progressBound) return
    progressBound = true
    try {
      await listen<TunnelProgress>('tunnels-progress', event => {
        const payload = event.payload
        if (
          payload.traceId === currentTraceId.value ||
          payload.traceId === viewTraceId.value
        ) {
          progress.value = payload
        }
      })
    } catch (error) {
      progressBound = false
      console.error('[tunnels] Écoute de la progression impossible :', error)
    }
  }

  return {
    // État de vue
    file,
    fileExists,
    loading,
    saving,
    validated,
    dirty,
    selectedTunnelId,
    boundsDraft,
    requirements,
    loadError,
    // État de file et support
    queue,
    currentTraceId,
    isRunning,
    progress,
    support,
    supportLoaded,
    lastError,
    lastErrorTraceId,
    // Getters
    displayFile,
    tunnels,
    groups,
    tunnelCount,
    selectedGroup,
    selectedTunnelIds,
    isPending,
    isPreparing,
    progressFor,
    outsideCoverageDataset,
    // Chargement / support
    loadOrCreate,
    refreshSupport,
    refreshRequirements,
    // Édition
    selectTunnel,
    createTunnel,
    draftTunnelBound,
    clearTunnelDraft,
    updateTunnelBound,
    deleteGroup,
    mergeWithPrevious,
    splitGroup,
    save,
    reset,
    // File
    detect,
    request,
    startCatchUp,
    bindProgress,
  }
})
