/**
 * Store Pinia du nettoyage de trace GPX.
 *
 * Gère l'état de la vue `/nettoyage` : pipeline de **3 étapes séquentielles**
 * (pts hors trace → ronds-points → aller/retour), détection par phase,
 * corrections appliquées par l'utilisateur, sauvegardes partielles et
 * **validation d'étape**.
 *
 * Principe : la détection (backend) ne fait que **proposer** des cas ; chaque
 * cas doit être **validé par l'utilisateur** (`corrected` ou `kept`) avant de
 * passer au suivant. La validation d'une étape réécrit le GPX (entrée de
 * l'étape suivante) une fois **tous** les cas de la phase validés.
 *
 * Pattern Setup Store (cf. traces.ts, edition.ts). Toute communication avec le
 * disque passe par les commandes Tauri (`detect_trace_anomalies`,
 * `get_cleaning_state`, `save_cleaning_state`, `validate_phase`,
 * `reset_cleaning`).
 */

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useTracesStore } from './traces'
import { useSettingsStore } from './settings'

// --- Types (miroir exact des structs Rust cleaning.rs, snake_case) ---

export type CleaningCaseKind = 'spike' | 'roundabout' | 'out_and_back' | 'manual'

export type CleaningCaseState = 'pending' | 'corrected' | 'kept'

/** Phase du pipeline de nettoyage (identifiants partagés avec le backend). */
export type CleaningPhaseId = 'spike' | 'roundabout' | 'out_and_back'

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
  /** Angle cumulé **signé** d'un rond-point (degrés) — 0 hors rond-point. */
  total_angle_deg: number
  suggested_delete_ranges: [number, number][]
  state: CleaningCaseState
  correction: Correction
}

/** État complet du nettoyage d'une phase (persisté par `save_cleaning_state`). */
export interface CleaningState {
  trace_id: string
  tolerance_deg: number
  /** Phase en cours (« spike » | « roundabout » | « out_and_back »). */
  phase: CleaningPhaseId
  cases: CleaningCase[]
}

/** Paramètres de détection des ronds-points (`Nettoyage.RondPoints.*`). */
export interface RoundaboutParams {
  angle_min_deg: number
  points_min: number
  points_max: number
  angle_seuil_deg: number
  marge_points: number
}

/** Définition d'une étape du pipeline de nettoyage. */
export interface CleaningPhaseDef {
  id: CleaningPhaseId
  num: number
  label: string
  icon: string
}

/** Les 3 étapes, dans l'ordre du pipeline. */
export const CLEANING_PHASES: CleaningPhaseDef[] = [
  { id: 'spike', num: 1, label: 'Pts hors trace', icon: 'mdi-dots-hexagon' },
  { id: 'roundabout', num: 2, label: 'Rond-Points', icon: 'mdi-rotate-360' },
  { id: 'out_and_back', num: 3, label: 'Aller/Retour', icon: 'mdi-arrow-u-left-bottom' },
]

/** Étape suivante dans le pipeline (l'étape 3 est la dernière). */
export const CLEANING_PHASE_NEXT: Record<CleaningPhaseId, CleaningPhaseId> = {
  spike: 'roundabout',
  roundabout: 'out_and_back',
  out_and_back: 'out_and_back',
}

/** Valeurs par défaut des paramètres de détection des ronds-points. */
export const DEFAULT_ROUNDABOUT_PARAMS: RoundaboutParams = {
  angle_min_deg: 5,
  points_min: 5,
  points_max: 50,
  angle_seuil_deg: 210,
  marge_points: 5,
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
  /** Phase du pipeline en cours (reflet de `cleaning_phase` de la trace). */
  const currentPhase = ref<CleaningPhaseId>('spike')
  /** Paramètres de détection des ronds-points (Nettoyage.RondPoints.*). */
  const roundaboutParams = ref<RoundaboutParams>({ ...DEFAULT_ROUNDABOUT_PARAMS })
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

  /**
   * Borne de début de la zone affichée du cas courant. Pour un rond-point, la
   * zone est **élargie de la marge** (`Nettoyage.RondPoints.margePoints`)
   * avant/après le segment détecté — l'utilisateur voit les points de contexte
   * pour supprimer/déplacer le tour excédentaire. Sinon, la zone = `[start, end]`.
   */
  const zoneStart = computed(() => {
    const c = currentCase.value
    if (!c) return 0
    if (c.kind === 'roundabout')
      return Math.max(0, c.start_index - roundaboutParams.value.marge_points)
    return c.start_index
  })

  /** Borne de fin de la zone affichée du cas courant (voir `zoneStart`). */
  const zoneEnd = computed(() => {
    const c = currentCase.value
    if (!c) return 0
    if (c.kind === 'roundabout')
      return Math.min(
        Math.max(0, points.value.length - 1),
        c.end_index + roundaboutParams.value.marge_points,
      )
    return c.end_index
  })

  /** Zone d'intérêt du cas courant (points de la trace, index originaux). */
  const currentZone = computed(() => {
    const c = currentCase.value
    if (!c || points.value.length === 0) return []
    return points.value.slice(zoneStart.value, zoneEnd.value + 1)
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
    const start = Math.max(0, zoneStart.value - 1)
    const end = Math.min(pts.length - 1, zoneEnd.value + 1)

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

  /** Lit la tolérance de cap et les paramètres ronds-points depuis les réglages. */
  async function readParams(): Promise<void> {
    try {
      const raw = await settingsStore.getSettingValue('Nettoyage.Cap.toleranceDeg')
      toleranceDeg.value = typeof raw === 'number' ? raw : 5.0
    } catch {
      toleranceDeg.value = 5.0
    }
    const keys = Object.keys(DEFAULT_ROUNDABOUT_PARAMS) as (keyof RoundaboutParams)[]
    const paths: Record<keyof RoundaboutParams, string> = {
      angle_min_deg: 'Nettoyage.RondPoints.angleMinDeg',
      points_min: 'Nettoyage.RondPoints.pointsMin',
      points_max: 'Nettoyage.RondPoints.pointsMax',
      angle_seuil_deg: 'Nettoyage.RondPoints.angleSeuilDeg',
      marge_points: 'Nettoyage.RondPoints.margePoints',
    }
    for (const k of keys) {
      try {
        const raw = await settingsStore.getSettingValue(paths[k])
        if (typeof raw === 'number') roundaboutParams.value[k] = raw
      } catch {
        // repli sur la valeur par défaut
      }
    }
  }

  /** Phase de départ depuis les métadonnées persistées de la trace. */
  function initialPhase(id: string): CleaningPhaseId {
    const p = tracesStore.traces.find(t => t.id === id)?.cleaning_phase
    return p === 'spike' || p === 'roundabout' || p === 'out_and_back' ? p : 'spike'
  }

  /**
   * Charge l'état de nettoyage : paramètres, points bruts et état de la **phase
   * courante** (fichier de travail s'il existe, sinon détection fraîche).
   *
   * **Auto-validation des étapes vides** : une étape qui ne détecte aucune
   * anomalie est validée automatiquement (avancement de phase sans réécriture
   * du GPX), jusqu'à l'étape 3 « Aller/Retour » — non implémentée, simple
   * emplacement dans la toolbar (on ne lance aucune détection à ce stade).
   */
  async function load(traceId?: string) {
    const id = traceId ?? selectedTraceId.value
    if (!id) return
    selectedTraceId.value = id
    loading.value = true
    try {
      await readParams()
      const pts = await tracesStore.getTracePoints(id)
      points.value = pts.map(p => ({ lat: p.lat, lon: p.lon, alt: p.alt }))

      let phase = initialPhase(id)
      let st: CleaningState | null = null
      for (let guard = 0; guard < CLEANING_PHASES.length; guard++) {
        if (phase === 'out_and_back') {
          // Étape 3 : non implémentée — état vide, aucune détection.
          st = { trace_id: id, tolerance_deg: toleranceDeg.value, phase, cases: [] }
          break
        }
        st = await invoke<CleaningState>('get_cleaning_state', {
          traceId: id,
          phase,
          toleranceDeg: toleranceDeg.value,
          roundaboutParams: roundaboutParams.value,
        })
        currentPhase.value = phase
        if (st.cases.length > 0) break
        // Étape sans anomalie → auto-validation (avancement de phase).
        await invoke('validate_phase', {
          traceId: id,
          phase,
          stateJson: JSON.parse(JSON.stringify(st)),
        })
        phase = CLEANING_PHASE_NEXT[phase]
      }

      state.value = st
      currentPhase.value = phase
      // Repositionner sur le premier cas non validé (ou le premier cas).
      const firstPending = st?.cases.findIndex(c => c.state === 'pending') ?? -1
      currentCaseIndex.value = firstPending >= 0 ? firstPending : 0
      // La phase a pu avancer (auto-validation) → rafraîchir le registre.
      await tracesStore.loadTraces()
    } finally {
      loading.value = false
    }
  }

  /**
   * Navigue vers une **étape précise** du pipeline (widget de la toolbar), sans
   * auto-validation : la détection de cette étape sur le GPX courant est
   * affichée telle quelle. L'étape 3 (Aller/Retour) n'affiche que son
   * emplacement (état vide, aucune détection).
   */
  async function goToPhase(phase: CleaningPhaseId) {
    const id = selectedTraceId.value
    if (!id) return
    loading.value = true
    try {
      await readParams()
      if (phase === 'out_and_back') {
        // Étape 3 : emplacement seul — état vide.
        currentPhase.value = phase
        state.value = { trace_id: id, tolerance_deg: toleranceDeg.value, phase, cases: [] }
        currentCaseIndex.value = 0
        return
      }
      const pts = await tracesStore.getTracePoints(id)
      points.value = pts.map(p => ({ lat: p.lat, lon: p.lon, alt: p.alt }))
      const st = await invoke<CleaningState>('get_cleaning_state', {
        traceId: id,
        phase,
        toleranceDeg: toleranceDeg.value,
        roundaboutParams: roundaboutParams.value,
      })
      currentPhase.value = phase
      state.value = st
      const firstPending = st.cases.findIndex(c => c.state === 'pending')
      currentCaseIndex.value = firstPending >= 0 ? firstPending : 0
    } finally {
      loading.value = false
    }
  }

  /** Étape validée ? (d'après `cleaning_phase` persistée de la trace.) */
  function phaseValidated(phaseId: CleaningPhaseId): boolean {
    const tp = tracesStore.traces.find(t => t.id === selectedTraceId.value)?.cleaning_phase
    if (phaseId === 'spike') return tp === 'roundabout' || tp === 'out_and_back'
    if (phaseId === 'roundabout') return tp === 'out_and_back'
    return false // étape 3 : jamais validée
  }

  /**
   * Phase **réelle** du pipeline (d'après `cleaning_phase` de la trace) : la
   * première étape non encore validée. Distincte de `currentPhase` (l'étape
   * affichée) quand l'utilisateur revient consulter une étape déjà franchie.
   */
  const pipelinePhase = computed<CleaningPhaseId>(() => {
    for (const p of CLEANING_PHASES) {
      if (!phaseValidated(p.id)) return p.id
    }
    return 'out_and_back'
  })

  /**
   * L'étape affichée peut-elle être **validée** ? Il faut qu'elle soit l'étape
   * courante du pipeline (pas déjà franchie) et que tous ses cas soient validés.
   * On ne re-valide jamais une étape déjà validée.
   */
  const canValidatePhase = computed(
    () => currentPhase.value !== 'out_and_back' && !phaseValidated(currentPhase.value) && allValidated.value,
  )

  /**
   * Re-détecte la phase courante avec les paramètres actuels. Les cas **déjà
   * validés** dont un apex subsiste dans la nouvelle détection sont conservés
   * (état + corrections) ; les autres cas repartent de la détection.
   */
  async function reDetect() {
    const id = selectedTraceId.value
    if (!id || !state.value) return
    loading.value = true
    try {
      await readParams()
      const fresh = await invoke<CleaningCase[]>('detect_trace_anomalies', {
        traceId: id,
        phase: currentPhase.value,
        toleranceDeg: toleranceDeg.value,
        roundaboutParams: roundaboutParams.value,
      })
      const previous = state.value.cases
      // Les cas déjà validés dont la **zone** recouvre un cas re-détecté du
      // même type sont conservés (état + corrections). La correspondance par
      // chevauchement de zone remplace l'appariement par apex, qui ne
      // fonctionne pas pour les ronds-points (apex_indices vide).
      const merged = fresh.map(freshCase => {
        const prev = previous.find(
          p =>
            p.state !== 'pending' &&
            p.kind === freshCase.kind &&
            p.start_index <= freshCase.end_index &&
            freshCase.start_index <= p.end_index,
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

  /** Tous les points de la zone affichée du cas courant sont-ils marqués à
   * supprimer ? (La zone est élargie de la marge pour les ronds-points.) */
  function isZoneFullyDeleted(): boolean {
    const c = currentCase.value
    if (!c) return false
    const len = zoneEnd.value - zoneStart.value + 1
    let count = 0
    for (const [from, to] of c.correction.delete_ranges) {
      const lo = Math.max(from, zoneStart.value)
      const hi = Math.min(to, zoneEnd.value)
      if (lo <= hi) count += hi - lo + 1
    }
    return count >= len
  }

  /**
   * Sélectionne (`true`) ou désélectionne (`false`) **tous** les points de la
   * zone affichée du cas courant (case à cocher d'en-tête du tableau).
   */
  function setZoneDeleted(deleted: boolean) {
    const c = currentCase.value
    if (!c) return
    if (deleted) {
      addDeleteRange(zoneStart.value, zoneEnd.value)
    } else {
      // Tout remettre : retirer les index de la zone des plages de suppression.
      const kept = flattenRanges(c.correction.delete_ranges).filter(
        i => i < zoneStart.value || i > zoneEnd.value,
      )
      c.correction.delete_ranges = toRanges(kept)
    }
  }

  /**
   * Vide les corrections du cas courant (retour à l'état proposé) **et remet le
   * cas à « À traiter »** : si le cas était validé (corrigé ou faux positif), il
   * redevient `pending` — les boutons Valider / Faux positif se réactivent et la
   * trace redevient non finalisable tant que le cas n'est pas re-validé.
   */
  function clearCorrection() {
    const c = currentCase.value
    if (!c) return
    c.correction = { delete_ranges: [], moved_points: [] }
    c.state = 'pending'
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
      total_angle_deg: 0,
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

  /** Sauvegarde partielle du travail de la phase courante (le GPX reste intact). */
  async function save(): Promise<void> {
    const id = selectedTraceId.value
    if (!id || !state.value) return
    state.value.tolerance_deg = toleranceDeg.value
    state.value.phase = currentPhase.value
    await invoke('save_cleaning_state', {
      traceId: id,
      phase: currentPhase.value,
      stateJson: JSON.parse(JSON.stringify(state.value)),
    })
    await tracesStore.loadTraces()
  }

  /**
   * Valide l'**étape courante** : applique les corrections validées de la
   * phase, réécrit le GPX (qui devient l'entrée de l'étape suivante) et avance
   * la phase. Ne doit être appelé que quand tous les cas de la phase sont
   * validés (bouton inactif sinon). L'étape 3 n'est pas validable (non
   * implémentée).
   */
  async function validatePhase(): Promise<boolean> {
    const id = selectedTraceId.value
    if (!id || !state.value) return false
    if (!allValidated.value) return false
    if (currentPhase.value === 'out_and_back') return false
    state.value.tolerance_deg = toleranceDeg.value
    state.value.phase = currentPhase.value
    await invoke('validate_phase', {
      traceId: id,
      phase: currentPhase.value,
      stateJson: JSON.parse(JSON.stringify(state.value)),
    })
    // Réinitialiser l'état local, invalider la géométrie mise en cache (le GPX
    // source a changé) et recharger l'étape suivante (avec auto-validation des
    // étapes vides).
    state.value = null
    currentCaseIndex.value = 0
    tracesStore.invalidateGeometry(id)
    await load()
    return true
  }

  /** Abandonne les corrections en cours (retour à l'étape 1, `needs_review`). */
  async function reset(): Promise<void> {
    const id = selectedTraceId.value
    if (!id) return
    await invoke('reset_cleaning', { traceId: id })
    state.value = null
    currentCaseIndex.value = 0
    currentPhase.value = 'spike'
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
    currentPhase,
    toleranceDeg,
    roundaboutParams,
    loading,
    // État UI éphémère
    createMode,
    createStartIndex,
    movePointIndex,
    // Getters
    hasCases,
    currentCase,
    currentZone,
    zoneStart,
    zoneEnd,
    correctedZoneCoords,
    allValidated,
    validatedCount,
    isDeletedCount,
    pipelinePhase,
    canValidatePhase,
    // Actions
    selectTrace,
    load,
    goToPhase,
    phaseValidated,
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
    validatePhase,
    reset,
  }
})
