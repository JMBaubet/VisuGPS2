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

export type CleaningCaseKind = 'spike' | 'out_and_back' | 'manual'

export type CleaningCaseState = 'pending' | 'corrected' | 'kept'

/** Point de la trace déplacé géographiquement (index original + coordonnées). */
export interface MovedPoint {
  index: number
  lat: number
  lon: number
}

/** Corrections appliquées à un cas (index **originaux**, avant suppression). */
export interface Correction {
  delete_ranges: [number, number][]
  moved_points: MovedPoint[]
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

  // --- État UI éphémère (non persisté) ---

  /** Mode « création d'anomalie » : deux clics sur la carte délimitent un cas. */
  const createMode = ref(false)
  /** Index (original) du point de début posé par le premier clic, ou null. */
  const createStartIndex = ref<number | null>(null)
  /** Point en cours de déplacement sur la carte (index original), ou null. */
  const movePointIndex = ref<number | null>(null)

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

  /**
   * Coordonnées `[lon, lat]` du **segment courant après application des
   * corrections** : les points marqués à supprimer sont retirés et les points
   * déplacés sont positionnés à leurs nouvelles coordonnées. Un point de marge
   * avant/après la zone est inclus pour le raccord visuel. Permet d'afficher
   * l'impact réel d'une modification sur le linestring.
   */
  const correctedZoneCoords = computed<number[][]>(() => {
    const c = currentCase.value
    const pts = points.value
    if (!c || pts.length === 0) return []
    const start = Math.max(0, c.start_index - 1)
    const end = Math.min(pts.length - 1, c.end_index + 1)

    const movedByIndex = new Map<number, MovedPoint>()
    for (const mp of c.correction.moved_points) movedByIndex.set(mp.index, mp)

    const out: number[][] = []
    for (let i = start; i <= end; i++) {
      if (isDeleted(i)) continue
      const p = movedByIndex.get(i) ?? pts[i]
      out.push([p.lon, p.lat])
    }
    return out
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
      // Les cas **manuels** validés ne sont jamais re-détectés : on les
      // conserve tels quels (état + corrections) pour ne rien perdre.
      for (const prev of previous) {
        if (prev.state !== 'pending' && prev.kind === 'manual' && !merged.includes(prev)) {
          merged.push(prev)
        }
      }
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

  /**
   * Supprime une plage entière [from, to] (index originaux) dans le cas
   * courant. Tous les index de `from` à `to` sont marqués (pas seulement les
   * bornes) — la compaction en plages est ensuite faite par `toRanges`.
   */
  function addDeleteRange(from: number, to: number) {
    const c = currentCase.value
    if (!c || from > to) return
    const idx: number[] = []
    for (let i = from; i <= to; i++) idx.push(i)
    c.correction.delete_ranges = toRanges([...flattenRanges(c.correction.delete_ranges), ...idx])
  }

  /** Tous les points de la zone du cas courant sont-ils marqués à supprimer ? */
  function isZoneFullyDeleted(): boolean {
    const c = currentCase.value
    if (!c) return false
    const len = c.end_index - c.start_index + 1
    let count = 0
    for (const [from, to] of c.correction.delete_ranges) {
      const lo = Math.max(from, c.start_index)
      const hi = Math.min(to, c.end_index)
      if (lo <= hi) count += hi - lo + 1
    }
    return count >= len
  }

  /**
   * Sélectionne (`true`) ou désélectionne (`false`) **tous** les points du
   * segment courant (case à cocher d'en-tête du tableau des points).
   */
  function setZoneDeleted(deleted: boolean) {
    const c = currentCase.value
    if (!c) return
    if (deleted) {
      addDeleteRange(c.start_index, c.end_index)
    } else {
      // Tout remettre : retirer les index de la zone des plages de suppression.
      const kept = flattenRanges(c.correction.delete_ranges).filter(
        i => i < c.start_index || i > c.end_index,
      )
      c.correction.delete_ranges = toRanges(kept)
    }
  }

  /** Vide les corrections du cas courant (retour à l'état proposé). */
  function clearCorrection() {
    const c = currentCase.value
    if (!c) return
    c.correction = { delete_ranges: [], moved_points: [] }
    movePointIndex.value = null
  }

  /** Vrai si le point (index original) est supprimé par une correction du cas courant. */
  function isDeleted(index: number): boolean {
    const c = currentCase.value
    if (!c) return false
    return c.correction.delete_ranges.some(r => r[0] <= index && index <= r[1])
  }

  /** Point déplacé du cas courant pour un index original (ou null). */
  function isMoved(index: number): MovedPoint | null {
    const c = currentCase.value
    if (!c) return null
    return c.correction.moved_points.find(mp => mp.index === index) ?? null
  }

  /** Déplace un point (index original) vers de nouvelles coordonnées. */
  function setMovedPoint(index: number, lat: number, lon: number) {
    const c = currentCase.value
    if (!c) return
    // Garde-fou : index invalide (négatif ou hors bornes) → ignoré.
    if (!Number.isInteger(index) || index < 0 || index >= points.value.length) return
    const existing = c.correction.moved_points.find(mp => mp.index === index)
    if (existing) {
      existing.lat = lat
      existing.lon = lon
    } else {
      c.correction.moved_points.push({ index, lat, lon })
    }
  }

  /** Annule le déplacement d'un point. */
  function clearMovedPoint(index: number) {
    const c = currentCase.value
    if (!c) return
    c.correction.moved_points = c.correction.moved_points.filter(mp => mp.index !== index)
  }

  /** Démarre/arrête le déplacement d'un point sur la carte. */
  function startMovePoint(index: number) {
    movePointIndex.value = index
  }

  function stopMovePoint() {
    movePointIndex.value = null
  }

  // --- Création manuelle d'un cas ---

  /** Active/désactive le mode « création d'anomalie » (2 clics sur la carte). */
  function toggleCreateMode() {
    createMode.value = !createMode.value
    createStartIndex.value = null
  }

  /** Abandonne la création en cours. */
  function cancelCreate() {
    createMode.value = false
    createStartIndex.value = null
  }

  /**
   * Crée un cas **manuel** à partir d'une plage d'index originaux désignée sur
   * la carte (2 clics). Le cas est ajouté à la liste et devient courant ; il
   * peut être supprimé avant validation.
   */
  function createManualCase(start: number, end: number) {
    const st = state.value
    if (!st) return
    const lo = Math.min(start, end)
    const hi = Math.max(start, end)
    if (lo < 0 || hi >= points.value.length || lo === hi) {
      cancelCreate()
      return
    }
    const c: CleaningCase = {
      // Identifiant unique (horodatage) — les cas manuels ne proviennent pas
      // de la détection et ne doivent jamais entrer en collision d'id.
      id: `manual-${Date.now()}`,
      kind: 'manual',
      start_index: lo,
      end_index: hi,
      apex_indices: [],
      bearing_delta_deg: 0,
      suggested_delete_ranges: [],
      state: 'pending',
      correction: { delete_ranges: [], moved_points: [] },
    }
    st.cases.push(c)
    currentCaseIndex.value = st.cases.length - 1
    cancelCreate()
  }

  /**
   * Supprime un cas de la liste. Un cas « modification de segment » (manuel)
   * peut être supprimé **même après validation** ; les cas détectés
   * automatiquement ne sont supprimables que tant qu'ils ne sont pas validés.
   */
  function removeCase(index: number): boolean {
    const st = state.value
    if (!st || index < 0 || index >= st.cases.length) return false
    const c = st.cases[index]
    if (c.state !== 'pending' && c.kind !== 'manual') return false
    st.cases.splice(index, 1)
    currentCaseIndex.value = Math.min(currentCaseIndex.value, Math.max(0, st.cases.length - 1))
    cancelCreate()
    stopMovePoint()
    return true
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
      c.correction = { delete_ranges: [], moved_points: [] }
    }
    stopMovePoint()
    cancelCreate()
    // Passer au prochain cas non validé.
    const cases = state.value?.cases ?? []
    const next = cases.findIndex((cc, i) => i > currentCaseIndex.value && cc.state === 'pending')
    if (next >= 0) currentCaseIndex.value = next
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
    // État UI éphémère
    createMode,
    createStartIndex,
    movePointIndex,
    // Getters
    hasCases,
    currentCase,
    currentZone,
    correctedZoneCoords,
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
    isZoneFullyDeleted,
    setZoneDeleted,
    isDeleted,
    isMoved,
    setMovedPoint,
    clearMovedPoint,
    startMovePoint,
    stopMovePoint,
    toggleCreateMode,
    cancelCreate,
    createManualCase,
    removeCase,
    validateCurrentCase,
    save,
    finalize,
    reset,
  }
})
