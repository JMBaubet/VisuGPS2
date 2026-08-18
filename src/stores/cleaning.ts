/**
 * Store Pinia du nettoyage de trace GPX.
 *
 * Gère l'état de la vue `/nettoyage` : détection des anomalies (points hors
 * trace, aller-retours), corrections appliquées par l'utilisateur, sauvegardes
 * partielles et finalisation.
 *
 * Principe : la détection (backend) ne fait que **proposer** des cas ; chaque
 * cas doit être **validé par l'utilisateur** (`corrected` ou `kept`) avant de
 * passer au suivant. Le GPX original n'est remplacé qu'à la finalisation, une
 * fois **tous** les cas validés.
 *
 * Pattern Setup Store (cf. traces.ts, edition.ts). Toute communication avec le
 * disque passe par les commandes Tauri (`detect_trace_anomalies`,
 * `get_cleaning_state`, `save_cleaning_state`, `finalize_cleaning`,
 * `reset_cleaning`).
 */

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useTracesStore } from './traces'
import { useSettingsStore } from './settings'

// --- Types (miroir exact des structs Rust cleaning.rs, snake_case) ---

export type CleaningCaseKind = 'spike' | 'out_and_back' | 'parallel'

export type CleaningCaseState = 'pending' | 'corrected' | 'kept'

/** Point à insérer dans la trace, positionné après l'index original `after_index`. */
export interface InsertPoint {
  after_index: number
  lat: number
  lon: number
  ele: number | null
  time: string | null
}

/** Corrections appliquées à un cas (index **originaux**, avant suppression). */
export interface Correction {
  delete_ranges: [number, number][]
  insert_points: InsertPoint[]
}

/** Un cas d'anomalie à traiter. */
export interface CleaningCase {
  id: string
  kind: CleaningCaseKind
  start_index: number
  end_index: number
  apex_indices: number[]
  bearing_delta_deg: number
  suggested_delete_ranges: [number, number][]
  state: CleaningCaseState
  correction: Correction
}

/** État complet du nettoyage d'une trace (persisté par `save_cleaning_state`). */
export interface CleaningState {
  trace_id: string
  tolerance_deg: number
  cases: CleaningCase[]
}

// --- Store ---

export const useCleaningStore = defineStore('cleaning', () => {
  const tracesStore = useTracesStore()
  const settingsStore = useSettingsStore()

  /** Trace sélectionnée (posée avant la navigation vers /nettoyage). */
  const selectedTraceId = ref<string | null>(null)
  /** État complet (détection + décisions utilisateur). */
  const state = ref<CleaningState | null>(null)
  /** Points bruts de la trace (lat/lon/alt/distance), index = index GPX. */
  const points = ref<{ lat: number; lon: number; alt: number | null }[]>([])
  /** Index du cas courant dans la liste. */
  const currentCaseIndex = ref(0)
  /** Tolérance de cap (degrés) — paramètre Nettoyage.Cap.toleranceDeg. */
  const toleranceDeg = ref(5.0)
  const loading = ref(false)

  // --- Getters ---

  const hasCases = computed(() => (state.value?.cases.length ?? 0) > 0)

  const currentCase = computed<CleaningCase | null>(() => {
    const cases = state.value?.cases
    if (!cases || cases.length === 0) return null
    const idx = Math.min(currentCaseIndex.value, cases.length - 1)
    return cases[idx]
  })

  /** Zone d'intérêt du cas courant (points de la trace, index originaux). */
  const currentZone = computed(() => {
    const c = currentCase.value
    if (!c || points.value.length === 0) return []
    return points.value.slice(c.start_index, c.end_index + 1)
  })

  /** true quand tous les cas ont été validés par l'utilisateur. */
  const allValidated = computed(() => {
    const cases = state.value?.cases
    if (!cases || cases.length === 0) return false
    return cases.every(c => c.state !== 'pending')
  })

  const validatedCount = computed(
    () => state.value?.cases.filter(c => c.state !== 'pending').length ?? 0,
  )

  /** Nombre total de points marqués à supprimer (tous cas confondus). */
  const isDeletedCount = computed(() => {
    const cases = state.value?.cases ?? []
    return cases.reduce((acc, c) => {
      for (const [from, to] of c.correction.delete_ranges) acc += to - from + 1
      return acc
    }, 0)
  })

  // --- Sélection ---

  /** Sélectionne la trace à nettoyer (avant navigation vers /nettoyage). */
  function selectTrace(traceId: string) {
    selectedTraceId.value = traceId
  }

  /**
   * Charge l'état de nettoyage : tolérance paramétrée, points bruts et fichier
   * de travail s'il existe (corrections déjà en cours), sinon détection fraîche.
   */
  async function load(traceId?: string) {
    const id = traceId ?? selectedTraceId.value
    if (!id) return
    selectedTraceId.value = id
    loading.value = true
    try {
      // Tolérance paramétrée (Nettoyage.Cap.toleranceDeg).
      try {
        const raw = await settingsStore.getSettingValue('Nettoyage.Cap.toleranceDeg')
        toleranceDeg.value = typeof raw === 'number' ? raw : 5.0
      } catch {
        toleranceDeg.value = 5.0
      }

      const [pts, st] = await Promise.all([
        tracesStore.getTracePoints(id),
        invoke<CleaningState>('get_cleaning_state', {
          traceId: id,
          toleranceDeg: toleranceDeg.value,
        }),
      ])

      points.value = pts.map(p => ({ lat: p.lat, lon: p.lon, alt: p.alt }))
      state.value = st
      // Repositionner sur le premier cas non validé (ou le premier cas).
      const firstPending = st.cases.findIndex(c => c.state === 'pending')
      currentCaseIndex.value = firstPending >= 0 ? firstPending : 0
    } finally {
      loading.value = false
    }
  }

  /**
   * Re-détecte avec la tolérance courante. Les cas **déjà validés** dont un
   * apex subsiste dans la nouvelle détection sont conservés (état + corrections) ;
   * les autres cas repartent de la détection.
   */
  async function reDetect() {
    const id = selectedTraceId.value
    if (!id || !state.value) return
    loading.value = true
    try {
      const fresh = await invoke<CleaningCase[]>('detect_trace_anomalies', {
        traceId: id,
        toleranceDeg: toleranceDeg.value,
      })
      const previous = state.value.cases
      const merged = fresh.map(freshCase => {
        const prev = previous.find(
          p => p.state !== 'pending' && p.apex_indices.some(a => freshCase.apex_indices.includes(a)),
        )
        return prev ?? freshCase
      })
      state.value = { ...state.value, tolerance_deg: toleranceDeg.value, cases: merged }
      const firstPending = merged.findIndex(c => c.state === 'pending')
      currentCaseIndex.value = firstPending >= 0 ? firstPending : 0
    } finally {
      loading.value = false
    }
  }

  // --- Navigation ---

  function goToCase(index: number) {
    const cases = state.value?.cases
    if (!cases) return
    currentCaseIndex.value = Math.max(0, Math.min(index, cases.length - 1))
  }

  /**
   * Prépare la correction du cas courant à partir de la suggestion de
   * détection (pré-remplissage des plages de suppression). Idempotent.
   */
  function applySuggestion() {
    const c = currentCase.value
    if (!c) return
    if (c.correction.delete_ranges.length === 0 && c.suggested_delete_ranges.length > 0) {
      c.correction.delete_ranges = c.suggested_delete_ranges.map(r => [...r] as [number, number])
    }
  }

  /**
   * Bascule la suppression d'un point (index original) dans le cas courant.
   * Ajoute `[i, i]` s'il n'est pas supprimé, sinon le retire.
   */
  function toggleDeletePoint(index: number) {
    const c = currentCase.value
    if (!c) return
    const ranges = c.correction.delete_ranges
    const hit = ranges.find(r => r[0] <= index && index <= r[1])
    if (hit) {
      // Retirer le point : couper la plage [from, to] en deux si nécessaire.
      const flat = ranges.flatMap(([from, to]) => {
        const removed: number[] = []
        for (let i = from; i <= to; i++) if (i !== index) removed.push(i)
        return removed
      })
      c.correction.delete_ranges = toRanges(flat)
    } else {
      c.correction.delete_ranges = toRanges([...flattenRanges(ranges), index])
    }
  }

  /** Supprime une plage entière [from, to] (index originaux) dans le cas courant. */
  function addDeleteRange(from: number, to: number) {
    const c = currentCase.value
    if (!c) return
    c.correction.delete_ranges = toRanges([...flattenRanges(c.correction.delete_ranges), from, to])
  }

  /** Vide les corrections du cas courant (retour à l'état proposé). */
  function clearCorrection() {
    const c = currentCase.value
    if (!c) return
    c.correction = { delete_ranges: [], insert_points: [] }
  }

  /** Vrai si le point (index original) est supprimé par une correction du cas courant. */
  function isDeleted(index: number): boolean {
    const c = currentCase.value
    if (!c) return false
    return c.correction.delete_ranges.some(r => r[0] <= index && index <= r[1])
  }

  /**
   * Ajoute un point à insérer dans le cas courant. `afterIndex` = index du
   * point original après lequel insérer (déterminé par le clic carte).
   */
  function addInsertPoint(afterIndex: number, lat: number, lon: number) {
    const c = currentCase.value
    if (!c) return
    const prev = points.value[afterIndex]
    const next = points.value[afterIndex + 1]
    // Interpolation de l'élévation (et du temps) depuis les voisins.
    const ele =
      prev?.alt != null && next?.alt != null
        ? Math.round(((prev.alt + next.alt) / 2) * 10) / 10
        : prev?.alt ?? next?.alt ?? null
    c.correction.insert_points.push({
      after_index: afterIndex,
      lat,
      lon,
      ele,
      time: null,
    })
  }

  function removeInsertPoint(index: number) {
    const c = currentCase.value
    if (!c) return
    c.correction.insert_points.splice(index, 1)
  }

  function setInsertPointPosition(index: number, lat: number, lon: number) {
    const c = currentCase.value
    if (!c) return
    const p = c.correction.insert_points[index]
    if (p) {
      p.lat = lat
      p.lon = lon
    }
  }

  // --- Validation ---

  /**
   * Valide le cas courant. La validation est **manuelle et obligatoire** :
   * `corrected` (les corrections sont appliquées à la finalisation) ou `kept`
   * (faux positif — rien n'est modifié). Ne passe au cas suivant qu'après.
   */
  function validateCurrentCase(issue: 'corrected' | 'kept') {
    const c = currentCase.value
    if (!c) return
    if (issue === 'corrected') {
      applySuggestion()
      c.state = 'corrected'
    } else {
      c.state = 'kept'
      c.correction = { delete_ranges: [], insert_points: [] }
    }
    // Passer au prochain cas non validé.
    const cases = state.value?.cases ?? []
    const next = cases.findIndex((cc, i) => i > currentCaseIndex.value && cc.state === 'pending')
    if (next >= 0) currentCaseIndex.value = next
  }

  /** Bascule un cas en mode « ajout de points » (type parallel). */
  function setCaseKind(kind: CleaningCaseKind) {
    const c = currentCase.value
    if (c) c.kind = kind
  }

  // --- Persistance ---

  /** Sauvegarde partielle du travail (le GPX original reste intact). */
  async function save(): Promise<void> {
    const id = selectedTraceId.value
    if (!id || !state.value) return
    state.value.tolerance_deg = toleranceDeg.value
    await invoke('save_cleaning_state', {
      traceId: id,
      stateJson: JSON.parse(JSON.stringify(state.value)),
    })
    await tracesStore.loadTraces()
  }

  /**
   * Finalise le nettoyage : remplace le GPX original par la version nettoyée.
   * Ne doit être appelé que quand tous les cas sont validés (bouton inactif
   * sinon) — le backend refuse sinon. Retourne les métadonnées à jour.
   */
  async function finalize(): Promise<boolean> {
    const id = selectedTraceId.value
    if (!id || !state.value) return false
    if (!allValidated.value) return false
    state.value.tolerance_deg = toleranceDeg.value
    await invoke('finalize_cleaning', {
      traceId: id,
      stateJson: JSON.parse(JSON.stringify(state.value)),
    })
    // Réinitialiser l'état local, invalider la géométrie mise en cache (le GPX
    // source a changé) et recharger le registre (statut « clean »).
    state.value = null
    currentCaseIndex.value = 0
    tracesStore.invalidateGeometry(id)
    await tracesStore.loadTraces()
    return true
  }

  /** Abandonne les corrections en cours (retour « needs_review »). */
  async function reset(): Promise<void> {
    const id = selectedTraceId.value
    if (!id) return
    await invoke('reset_cleaning', { traceId: id })
    state.value = null
    currentCaseIndex.value = 0
    await tracesStore.loadTraces()
  }

  // --- Utilitaires ---

  /** Aplati les plages en liste d'index. */
  function flattenRanges(ranges: [number, number][]): number[] {
    const out: number[] = []
    for (const [from, to] of ranges) {
      for (let i = from; i <= to; i++) out.push(i)
    }
    return out
  }

  /** Compacte une liste d'index en plages [from, to] ordonnées et disjointes. */
  function toRanges(indices: number[]): [number, number][] {
    if (indices.length === 0) return []
    const sorted = [...new Set(indices)].sort((a, b) => a - b)
    const ranges: [number, number][] = []
    let start = sorted[0]
    let prev = sorted[0]
    for (let i = 1; i < sorted.length; i++) {
      if (sorted[i] === prev + 1) {
        prev = sorted[i]
      } else {
        ranges.push([start, prev])
        start = sorted[i]
        prev = sorted[i]
      }
    }
    ranges.push([start, prev])
    return ranges
  }

  return {
    // État
    selectedTraceId,
    state,
    points,
    currentCaseIndex,
    toleranceDeg,
    loading,
    // Getters
    hasCases,
    currentCase,
    currentZone,
    allValidated,
    validatedCount,
    isDeletedCount,
    // Actions
    selectTrace,
    load,
    reDetect,
    goToCase,
    applySuggestion,
    toggleDeletePoint,
    addDeleteRange,
    clearCorrection,
    isDeleted,
    addInsertPoint,
    removeInsertPoint,
    setInsertPointPosition,
    validateCurrentCase,
    setCaseKind,
    save,
    finalize,
    reset,
  }
})
