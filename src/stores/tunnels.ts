/**
 * Store Pinia du module Tunnels.
 *
 * Deux responsabilités, comme le store Communes :
 * 1. **état de vue** de l'onglet Tunnels (`file`, brouillon de bornes,
 *    sélection, sauvegarde, validation) ;
 * 2. **file d'attente globale** de préparation silencieuse : une trace à la
 *    fois, téléchargement Geofabrik + détection, avec progression.
 *
 * La file est déclenchée après un import propre, après une validation d'audit,
 * au lancement sur les traces déjà propres, et par le bouton « Relancer » de
 * l'onglet. Elle ne montre aucune confirmation : la progression est exposée par
 * `progress` et l'état final par le registre (`tunnels_status`).
 */

import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

import {
  applyTunnelBounds,
  constrainTunnelBound,
  groupTunnels,
  MIN_TUNNEL_LENGTH_KM,
  tunnelRangeOverlaps,
} from '../utils/altimetrieTunnels'
import { useTracesStore } from './traces'

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

export const useTunnelsStore = defineStore('tunnels', () => {
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

  /** Traces dont la préparation a échoué dans cette session. */
  const failed = new Set<string>()
  /** Demandes explicites : elles ne sont pas écartées par l'état du registre. */
  const forced = new Set<string>()
  /** Prochaine préparation de ces traces ignore le plafond de taille. */
  const sizeLimitOverrides = new Set<string>()
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

  // ─── Chargement et support ─────────────────────────────────────────

  async function loadOrCreate(
    traceId: string,
    persistedValidated = false,
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
        validated.value = persistedValidated || loaded.tunnels.length === 0
      } else {
        file.value = emptyFile(traceId)
        fileExists.value = false
        validated.value = false
      }
      dirty.value = false
      selectedTunnelId.value = null
      boundsDraft.value = null
      void refreshRequirements(traceId)
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
      void refreshRequirements(traceId)
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

  async function detect(
    traceId: string,
    ignoreSizeLimit = false,
  ): Promise<TunnelPrepareResult> {
    return invoke<TunnelPrepareResult>('tunnels_prepare', {
      traceId,
      ignoreSizeLimit,
    })
  }

  /** Prépare une trace : téléchargements silencieux puis détection. */
  async function runQueue(): Promise<void> {
    if (isRunning.value) return
    isRunning.value = true
    const tracesStore = useTracesStore()

    try {
      while (queue.value.length > 0) {
        const traceId = queue.value.shift()
        if (!traceId || failed.has(traceId)) {
          forced.delete(traceId ?? '')
          sizeLimitOverrides.delete(traceId ?? '')
          continue
        }

        currentTraceId.value = traceId
        progress.value = null
        lastError.value = null
        lastErrorTraceId.value = null

        await tracesStore.loadTraces()
        const trace = tracesStore.traces.find(candidate => candidate.id === traceId)
        const isForced = forced.has(traceId)
        const ignoreSizeLimit = sizeLimitOverrides.has(traceId)
        forced.delete(traceId)
        sizeLimitOverrides.delete(traceId)

        if (!trace || trace.audit_status !== 'clean') continue
        if (
          !isForced &&
          (trace.tunnels_status === 'waiting_validate' ||
            trace.tunnels_status === 'success')
        ) {
          continue
        }

        try {
          const result = await detect(traceId, ignoreSizeLimit)
          failed.delete(traceId)
          if (viewTraceId.value === traceId) {
            file.value = result.file
            fileExists.value = true
            validated.value = result.status === 'success'
            dirty.value = false
            selectedTunnelId.value = null
            boundsDraft.value = null
          }
          await tracesStore.loadTraces()
          if (viewTraceId.value === traceId) {
            await refreshRequirements(traceId)
          }
        } catch (error) {
          const reason = errorMessage(error)
          failed.add(traceId)
          lastError.value = reason
          lastErrorTraceId.value = traceId
          console.warn(
            `[tunnels] Préparation impossible pour ${traceId} : ${reason}`,
          )
          await tracesStore.loadTraces()
          if (viewTraceId.value === traceId) {
            await refreshRequirements(traceId)
          }
        } finally {
          progress.value = null
        }
      }
    } finally {
      isRunning.value = false
      currentTraceId.value = null
      progress.value = null
    }
  }

  /**
   * Demande explicite : relance même si le registre indique déjà une analyse.
   * La trace passe en tête de file. `ignoreSizeLimit` autorise le
   * téléchargement des PBF au-dessus du plafond.
   */
  function request(traceId: string, ignoreSizeLimit = false): void {
    if (currentTraceId.value === traceId) return
    failed.delete(traceId)
    forced.add(traceId)
    if (ignoreSizeLimit) sizeLimitOverrides.add(traceId)
    else sizeLimitOverrides.delete(traceId)
    if (queue.value.includes(traceId)) return
    queue.value.unshift(traceId)
    void runQueue()
  }

  /**
   * Rattrapage au lancement : traces propres sans analyse validée. Ne fait rien
   * tant que le support OSM n'est pas configuré et lisible.
   */
  async function startCatchUp(): Promise<void> {
    await refreshSupport()
    if (
      !support.value.active ||
      !support.value.configured ||
      !support.value.readable
    ) {
      return
    }

    const tracesStore = useTracesStore()
    await tracesStore.loadTraces()
    const pending = tracesStore.traces
      .filter(
        trace =>
          trace.audit_status === 'clean' &&
          trace.tunnels_status !== 'waiting_validate' &&
          trace.tunnels_status !== 'success' &&
          !failed.has(trace.id),
      )
      .map(trace => trace.id)

    if (pending.length === 0) return
    for (const traceId of pending) {
      if (!isPending(traceId)) queue.value.push(traceId)
    }
    void runQueue()
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
