// Store Pinia du module Audit GPX.
//
// Miroir exact des structs Rust définies dans src-tauri/src/gpx_audit/types.rs
// (elles-mêmes sérialisées en camelCase). Pattern Setup Store (conforme à
// docs/CONVENTIONS.md).
//
// Point de contrat : le store ne fait **aucun calcul métier**. Il délègue tout
// le travail lourd aux commandes Tauri (src-tauri/src/gpx_audit/commands.rs) et
// se contente de conserver l'état, d'exposer des getters dérivés et
// d'orchestrer les appels.
//
// État **archivé** (avenant à la décision 6) : le travail est écrit sur disque
// au fil des actions, via `audit_save_state` (`traces/{trace_id}/audit.json`).
// L'archive sert la **reprise** d'une session interrompue (`needs_review`) et la
// **consultation** d'un audit validé (`clean`) — en lecture seule, sans
// annulation possible : les enregistrements d'annulation n'ont de sens que dans
// la session qui les a produits.

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useUiStore } from './ui'

// ─── Types primaires ──────────────────────────────────────────────────

export type FindingKind = 'ar' | 'rp'
export type FindingStatus = 'pending' | 'corrected' | 'fp'
export type CorrectionType = 'delete' | 'route-car' | 'route-bike'
export type PartRole = 'warn' | 'info'

// ─── Structures ───────────────────────────────────────────────────────

export interface AuditPoint {
  id: number
  lat: number
  lon: number
  ele?: number | null
}

export interface LatLon {
  lat: number
  lon: number
}

export interface FindingPair {
  aid: number
  bid: number
  a: number
  b: number
  d: number
}

export interface FindingPart {
  s: number
  e: number
  role: PartRole
  text: string
}

export interface FindingContext {
  up: number | null
  dn: number | null
}

export interface FindingContextIds {
  up: number | null
  dn: number | null
}

export interface UndoDelete {
  type: 'delete'
  origPts: AuditPoint[]
  anchorLeftId: number | null
  anchorRightId: number | null
  firstNo: number
  absorbedFp: Finding[]
}

export interface UndoRoute {
  type: 'route'
  origPts: AuditPoint[]
  insertedIds: number[]
  routePts: LatLon[]
  startPt: LatLon
  endPt: LatLon
  firstNo: number
  absorbedFp: Finding[]
}

export type UndoRecord = UndoDelete | UndoRoute

export interface Finding {
  id: string
  kind: FindingKind
  label: string
  summary: string
  peak: number
  peakId: number
  pairs: FindingPair[]
  pairIdx: number[]

  // AR uniquement
  ecart: number | null
  d1: number | null
  d2: number | null

  // RP uniquement
  totalAngle: number | null
  turnText: string | null
  coreIds: number[]

  zoneIds: number[]
  ctxIds: FindingContextIds
  ctx: FindingContext
  parts: FindingPart[]

  status: FindingStatus
  correction: CorrectionType | null
  undo: UndoRecord | null

  // Champ technique côté front (non sérialisé par Rust).
  // Rempli par le composant carte après calcul des bornes LngLatBounds.
  _bounds?: unknown
}

export interface AuditParams {
  consolM: number
  tolDeg: number
  pairM: number
  maxpairs: number
  segM: number
  closeM: number
  angleDeg: number
}

export interface AuditDetectionResult {
  traceId: string
  points: AuditPoint[]
  totalDistanceM: number
  findings: Finding[]
  params: AuditParams
  durationMs: number
}

export interface AuditState {
  points: AuditPoint[]
  findings: Finding[]
}

// ─── Archive disque de l'audit (`traces/{trace_id}/audit.json`) ───────

/**
 * Miroir de `gpx_audit::archive::AuditArchive` : l'état complet d'un audit
 * (trace de travail, findings, traitements) tel qu'il est écrit sur disque.
 */
export interface AuditArchive {
  version: number
  traceId: string
  updatedAt: string
  validated: boolean
  params: AuditParams
  totalDistanceM: number
  points: AuditPoint[]
  findings: Finding[]
}

// ─── Éléments de rendu de la carte (commande `audit_map_overlay`) ─────

export interface RpAnchors {
  up: number
  dn: number
  natural: boolean
  center: LatLon | null
  radiusM: number | null
}

export interface LabelItem {
  index: number
  lat: number
  lon: number
  no: number
  cls: string
  dx: number
  dy: number
}

export interface MapOverlay {
  id: string
  anchors: RpAnchors | null
  labels: LabelItem[]
}

// ─── Aperçu d'une suppression (commande `audit_delete_preview`) ───────

export interface PreviewPointStyle {
  index: number
  kept: boolean
  isAnchor: boolean
}

export interface PreviewCursor {
  index: number
  lat: number
  lon: number
  /** « conservé » ou « bord de trace » (curseur AR aux extrémités). */
  title: string
}

// ─── Routage OpenRouteService (échanges et aperçu) ────────────────────

/** Un tracé obtenu d'OpenRouteService. */
export interface OrsRoute {
  coords: LatLon[]
  /** Longueur annoncée par ORS (m). */
  distance: number
  /** Durée annoncée par ORS (s). */
  duration: number
}

/** Aperçu de routage en attente de choix (IHM §7.3, §7.5). */
export interface RoutePreview {
  car: OrsRoute | null
  bike: OrsRoute | null
  /** Les deux tracés sont identiques (radios masqués). */
  identical: boolean
  /** Ancres conservées : la zone remplacée est `[start+1 .. end-1]`. */
  start: number
  end: number
}

export interface DeletePreview {
  /** Bornes effectives après clampage — réécrites dans les curseurs. */
  start: number
  end: number
  points: PreviewPointStyle[]
  join: LatLon[]
  red: LatLon[]
  cursors: PreviewCursor[]
  labels: LabelItem[]
  countStart: string
  countEnd: string
}

// ─── Store ────────────────────────────────────────────────────────────

export const useAuditStore = defineStore('audit', () => {
  // ─── State ─────────────────────────────────────────────────────
  const currentTraceId = ref<string | null>(null)
  const working = ref<AuditPoint[]>([])
  const findings = ref<Finding[]>([])
  const params = ref<AuditParams | null>(null)
  const selectedFindingId = ref<string | null>(null)
  const nextPointId = ref<number>(0)
  const analysisDurationMs = ref<number>(0)
  const totalDistanceM = ref<number>(0)
  /** Éléments de rendu des anomalies (ancres RP + étiquettes), par identifiant. */
  const overlays = ref<MapOverlay[]>([])
  /**
   * Aperçu de la correction de suppression en cours de réglage, ou `null`.
   * État UI éphémère : la trace de travail n'est jamais modifiée.
   */
  const deletePreview = ref<DeletePreview | null>(null)
  /**
   * Aperçu de routage en attente de choix, ou `null` (IHM §7.5).
   * Éphémère : la trace n'est modifiée qu'à l'application.
   */
  const routePreview = ref<RoutePreview | null>(null)
  /**
   * Bornes des curseurs de la vue de routage, ou `null` hors de cette vue.
   * La carte y pose ses marqueurs jaune/orange (IHM §6).
   */
  const routeRange = ref<[number, number] | null>(null)
  /**
   * `true` quand la vue affiche un audit **validé** (trace `clean`) : les
   * anomalies et leurs traitements sont visibles, mais plus modifiables.
   */
  const isConsultation = ref(false)
  /** Horodatage ISO de la dernière écriture de l'archive, ou `null`. */
  const archivedAt = ref<string | null>(null)

  // ─── Getters ───────────────────────────────────────────────────
  const pendingCount = computed(
    () => findings.value.filter((f) => f.status === 'pending').length,
  )
  const correctedCount = computed(
    () => findings.value.filter((f) => f.status === 'corrected').length,
  )
  const fpCount = computed(
    () => findings.value.filter((f) => f.status === 'fp').length,
  )
  const allProcessed = computed(() => pendingCount.value === 0)
  const hasWorkInProgress = computed(() =>
    findings.value.some((f) => f.status !== 'pending'),
  )
  const selectedFinding = computed(
    () => findings.value.find((f) => f.id === selectedFindingId.value) ?? null,
  )
  /**
   * `true` quand l'audit courant peut être appliqué (écriture du GPX) : au moins
   * un traitement effectué, plus aucune anomalie à traiter, et **session
   * modifiable** — un audit consulté ne se réapplique pas.
   */
  const canApply = computed(
    () => !isConsultation.value && hasWorkInProgress.value && allProcessed.value,
  )
  const totalFindings = computed(() => findings.value.length)
  /** Éléments de rendu de l'anomalie sélectionnée, ou `null`. */
  const selectedOverlay = computed(
    () => overlays.value.find((o) => o.id === selectedFindingId.value) ?? null,
  )
  /** Éléments de rendu d'une anomalie donnée (`null` si non calculés). */
  function overlayOf(findingId: string): MapOverlay | null {
    return overlays.value.find((o) => o.id === findingId) ?? null
  }

  // ─── Actions ───────────────────────────────────────────────────

  /**
   * Refuse toute modification d'un audit **consulté**.
   *
   * En consultation (trace déjà `clean`), on visualise les anomalies et les
   * corrections apportées : ni traitement, ni annulation, ni nouvelle
   * validation. Défense en profondeur — l'interface masque déjà ces actions.
   */
  function assertEditable(): void {
    if (isConsultation.value) {
      throw new Error(
        "Consultation d'un audit validé : les corrections ne peuvent plus être modifiées.",
      )
    }
  }

  /**
   * Lance la détection (AR + RP) sur la trace donnée.
   * Remplace intégralement l'état courant, puis **archive** le résultat.
   */
  async function runAudit(traceId: string, p: AuditParams): Promise<void> {
    const result = await invoke<AuditDetectionResult>('audit_run_detection', {
      traceId,
      params: p,
    })
    currentTraceId.value = traceId
    working.value = result.points
    findings.value = result.findings
    params.value = result.params
    analysisDurationMs.value = result.durationMs
    totalDistanceM.value = result.totalDistanceM
    selectedFindingId.value = null
    isConsultation.value = false
    // nextPointId initialisé au max + 1 pour les futures insertions.
    nextPointId.value =
      working.value.length > 0
        ? Math.max(...working.value.map((pt) => pt.id)) + 1
        : 0
    await loadMapOverlays()
    await persistOrWarn()
  }

  /**
   * Supprime une plage de points et met à jour le finding associé.
   * Rust valide la garde d'imbrication et la longueur restante.
   */
  async function applyDelete(
    findingId: string,
    ds: number,
    de: number,
  ): Promise<void> {
    assertEditable()
    if (!currentTraceId.value) {
      throw new Error("Aucune trace en cours d'audit.")
    }
    const state = await invoke<AuditState>('audit_apply_delete', {
      traceId: currentTraceId.value,
      points: working.value,
      findings: findings.value,
      findingId,
      ds,
      de,
      nextPointId: nextPointId.value,
    })
    working.value = state.points
    findings.value = state.findings
    deletePreview.value = null
    syncNextPointId()
    await loadMapOverlays()
    await persistOrWarn()
  }

  /**
   * Insère un tracé ORS et met à jour le finding associé.
   * `coords` contient le tracé complet renvoyé par ORS (les ancres sont
   * exclues côté Rust pour éviter les doublons).
   */
  async function applyRoute(
    findingId: string,
    start: number,
    end: number,
    coords: LatLon[],
    profile: 'driving-car' | 'cycling-road',
  ): Promise<void> {
    assertEditable()
    if (!currentTraceId.value) {
      throw new Error("Aucune trace en cours d'audit.")
    }
    const state = await invoke<AuditState>('audit_apply_route', {
      traceId: currentTraceId.value,
      points: working.value,
      findings: findings.value,
      findingId,
      start,
      end,
      coords,
      profile,
      nextPointId: nextPointId.value,
    })
    working.value = state.points
    findings.value = state.findings
    syncNextPointId()
    await loadMapOverlays()
    await persistOrWarn()
  }

  /** Marque un finding comme faux positif (trace inchangée). */
  async function markFp(findingId: string): Promise<void> {
    assertEditable()
    findings.value = await invoke<Finding[]>('audit_mark_fp', {
      findings: findings.value,
      findingId,
    })
    await loadMapOverlays()
    await persistOrWarn()
  }

  /** Retire le marqueur faux positif (retour à `pending`). */
  async function unmarkFp(findingId: string): Promise<void> {
    assertEditable()
    findings.value = await invoke<Finding[]>('audit_unmark_fp', {
      findings: findings.value,
      findingId,
    })
    await loadMapOverlays()
    await persistOrWarn()
  }

  /**
   * Annule la correction d'un finding (undo par instantané).
   * Rust refuse si la zone a été réutilisée par une correction ultérieure ou si
   * les ancres ont disparu.
   */
  async function undoCorrection(findingId: string): Promise<void> {
    assertEditable()
    if (!currentTraceId.value) {
      throw new Error("Aucune trace en cours d'audit.")
    }
    const state = await invoke<AuditState>('audit_undo_correction', {
      traceId: currentTraceId.value,
      points: working.value,
      findings: findings.value,
      findingId,
    })
    working.value = state.points
    findings.value = state.findings
    syncNextPointId()
    await loadMapOverlays()
    await persistOrWarn()
  }

  /**
   * Calcule l'aperçu d'une suppression sur `[start..=end]`.
   *
   * Sans effet sur la trace de travail : l'aperçu ne fait que **montrer** ce que
   * la validation produira. Recalculé à chaque mouvement de curseur, à l'image
   * de `delUpdate` / `rpDelUpdate` du HTML de référence.
   */
  async function previewDelete(start: number, end: number): Promise<void> {
    const finding = selectedFinding.value
    if (!finding || working.value.length === 0) {
      deletePreview.value = null
      return
    }
    deletePreview.value = await invoke<DeletePreview>('audit_delete_preview', {
      points: working.value,
      finding,
      start,
      end,
      closeM: params.value?.closeM ?? 15,
    })
  }

  /** Abandonne l'aperçu en cours (sortie de la vue de suppression). */
  function clearDeletePreview(): void {
    deletePreview.value = null
  }

  /** Mémorise l'aperçu de routage à afficher (IHM §7.5). */
  function setRoutePreview(preview: RoutePreview | null): void {
    routePreview.value = preview
  }

  /** Abandonne l'aperçu de routage (annulation, changement de vue). */
  function clearRoutePreview(): void {
    routePreview.value = null
  }

  /** Position des curseurs de la vue de routage sur la carte. */
  function setRouteRange(range: [number, number] | null): void {
    routeRange.value = range
  }

  /**
   * Valide l'audit : réécrit le GPX, pose `audit_status = "clean"`.
   * Point de non-retour. La vue doit fermer après succès.
   *
   * L'archive est réécrite juste après, marquée `validated` : elle devient la
   * pièce de consultation (et non plus une session de travail).
   */
  async function validateAndRewrite(): Promise<unknown> {
    assertEditable()
    if (!currentTraceId.value) {
      throw new Error("Aucune trace en cours d'audit.")
    }
    const result = await invoke('audit_validate', {
      traceId: currentTraceId.value,
      points: working.value,
      findings: findings.value,
    })
    await persistOrWarn(true)
    return result
  }

  /**
   * Charge l'état de travail depuis l'archive de la trace et **positionne le
   * mode** de la session.
   *
   * Le mode est posé même sans archive : une trace `clean` reste en lecture
   * seule quel que soit l'état de son archive (une trace validée ne se
   * réaudite pas).
   *
   * @param consultation `true` pour un audit **validé** (trace `clean`) :
   * l'état est restitué en lecture seule. `false` pour reprendre une session
   * interrompue (`needs_review`), qui reste entièrement modifiable.
   * @returns `false` si aucune archive exploitable n'existe — l'appelant
   * décide alors de relancer la détection (ou de n'afficher que la trace).
   */
  async function restore(traceId: string, consultation: boolean): Promise<boolean> {
    isConsultation.value = consultation
    const archive = await invoke<AuditArchive | null>('audit_load_archive', {
      traceId,
    })
    if (!archive) return false

    currentTraceId.value = traceId
    working.value = archive.points
    findings.value = archive.findings
    params.value = archive.params
    totalDistanceM.value = archive.totalDistanceM
    // La durée d'analyse n'est pas archivée : elle ne vaut que pour la
    // détection qui vient d'être jouée.
    analysisDurationMs.value = 0
    selectedFindingId.value = null
    deletePreview.value = null
    routePreview.value = null
    routeRange.value = null
    archivedAt.value = archive.updatedAt
    syncNextPointId()
    await loadMapOverlays()
    return true
  }

  /**
   * Écrit l'archive de l'état de travail courant (trace, findings, traitements).
   *
   * Sans état courant (aucune trace, aucun paramètre), l'écriture est sans
   * objet : c'est un no-op silencieux.
   */
  async function persist(validated = false): Promise<void> {
    if (!currentTraceId.value || !params.value) return
    archivedAt.value = await invoke<string>('audit_save_state', {
      traceId: currentTraceId.value,
      params: params.value,
      totalDistanceM: totalDistanceM.value,
      points: working.value,
      findings: findings.value,
      validated,
    })
  }

  /**
   * Archive sans jamais faire échouer le traitement : l'état en mémoire reste la
   * référence, mais un échec d'écriture est **signalé** — sans archive, le
   * travail n'est pas restituable après la fermeture.
   */
  async function persistOrWarn(validated = false): Promise<void> {
    try {
      await persist(validated)
    } catch (error) {
      const msg = typeof error === 'string' ? error : 'écriture impossible'
      useUiStore().showWarning(
        `Archive de l'audit non écrite (${msg}) — le travail ne sera pas restituable.`,
      )
    }
  }

  /**
   * Réinitialise intégralement le store.
   * Appelé à la sortie de la vue /audit (décision C.2).
   */
  function reset(): void {
    currentTraceId.value = null
    working.value = []
    findings.value = []
    params.value = null
    selectedFindingId.value = null
    nextPointId.value = 0
    analysisDurationMs.value = 0
    totalDistanceM.value = 0
    overlays.value = []
    deletePreview.value = null
    routePreview.value = null
    routeRange.value = null
    isConsultation.value = false
    archivedAt.value = null
  }

  /** Sélectionne ou désélectionne un finding. */
  function selectFinding(findingId: string | null): void {
    selectedFindingId.value = findingId
  }

  /**
   * Recalcule les éléments de rendu carte de toutes les anomalies.
   *
   * Le calcul est **lazy** (refait après chaque mutation de la trace de travail
   * ou des statuts), comme `rpAnchors` dans le HTML de référence : les indices
   * portés par les findings sont alors toujours à jour.
   */
  async function loadMapOverlays(): Promise<void> {
    if (!currentTraceId.value || working.value.length === 0) {
      overlays.value = []
      return
    }
    overlays.value = await invoke<MapOverlay[]>('audit_map_overlay', {
      points: working.value,
      findings: findings.value,
      closeM: params.value?.closeM ?? 15,
    })
  }

  /**
   * Recalcule `nextPointId` d'après le max des identifiants de la trace de
   * travail : Rust ne renvoie pas le compteur, et les identifiants ne sont
   * jamais recyclés (invariant C5).
   */
  function syncNextPointId(): void {
    nextPointId.value =
      working.value.length > 0
        ? Math.max(...working.value.map((pt) => pt.id)) + 1
        : nextPointId.value
  }

  return {
    // State
    currentTraceId,
    working,
    findings,
    params,
    selectedFindingId,
    nextPointId,
    analysisDurationMs,
    totalDistanceM,
    overlays,
    deletePreview,
    routePreview,
    routeRange,
    isConsultation,
    archivedAt,
    // Getters
    pendingCount,
    correctedCount,
    fpCount,
    allProcessed,
    hasWorkInProgress,
    selectedFinding,
    selectedOverlay,
    canApply,
    totalFindings,
    // Actions
    runAudit,
    restore,
    applyDelete,
    applyRoute,
    markFp,
    unmarkFp,
    undoCorrection,
    validateAndRewrite,
    reset,
    selectFinding,
    overlayOf,
    loadMapOverlays,
    previewDelete,
    clearDeletePreview,
    setRoutePreview,
    clearRoutePreview,
    setRouteRange,
  }
})
