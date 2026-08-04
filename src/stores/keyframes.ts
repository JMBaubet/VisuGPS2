/**
 * Store Pinia des keyframes et overrides de montage (Modes 1 & 2).
 *
 * Pattern Setup Store (cf. traces.ts, app.ts).
 *
 * Rôles :
 * - Charger / sauvegarder les fichiers de keyframes via les commandes Tauri
 *   du module `edition` (`get_raw_keyframes`, `save_raw_keyframes`,
 *   `get_montage_overrides`, `save_montage_overrides`, `delete_keyframes`).
 * - Exposer l'état de lecture (curseur temps, play/pause, vitesse) consommé
 *   par la timeline et la map 3D (live preview).
 * - Porter l'état du pré-calcul (progression, statut) affiché par l'overlay.
 *
 * Le moteur de fusion (blending) et la boucle de lecture (requestAnimationFrame)
 * vivent dans le composable `useKeyframeEngine`, qui lit/écrit ce store.
 */

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type {
  RawKeyframesFile,
  MontageOverridesFile,
  Override,
} from '../utils/keyframes'

/** Statut possible du pré-calcul d'une trace. */
export type PrecomputeStatus = 'idle' | 'needed' | 'running' | 'done' | 'error'

export const useKeyframesStore = defineStore('keyframes', () => {
  // --- État ---

  /** Identifiant de la trace actuellement chargée dans l'atelier d'édition. */
  const activeTraceId = ref<string | null>(null)

  /** Keyframes bruts (issus du pré-calcul), ou null si non calculés. */
  const rawKeyframes = ref<RawKeyframesFile | null>(null)

  /** Fichier d'overrides de montage (toujours défini une fois chargé). */
  const overrides = ref<MontageOverridesFile>({ overrides: [], messages: [], pois: [] })

  // --- État de lecture (consommé par la timeline et la map 3D) ---

  /** Position courante du curseur de lecture (en millisecondes). */
  const currentTime = ref(0)
  /** true si la lecture est en cours. */
  const isPlaying = ref(false)
  /** Facteur multiplicateur de la vitesse de lecture (1 = normal). */
  const playbackSpeed = ref(1)

  // --- État du pré-calcul ---

  /** Statut courant du pré-calcul pour la trace active. */
  const precomputeStatus = ref<PrecomputeStatus>('idle')
  /** Progression du pré-calcul (0 → 1), pilotée par le composable moteur. */
  const precomputeProgress = ref(0)
  /** Étape humainement lisible du pré-calcul (affichée dans l'overlay). */
  const precomputeStep = ref('')

  // --- Getters ---

  /** Durée totale (ms) de la trace active, ou 0 si non chargée. */
  const totalDuration = computed(() => rawKeyframes.value?.total_duration ?? 0)

  /** Distance totale (m) de la trace active, ou 0 si non chargée. */
  const totalDistance = computed(() => rawKeyframes.value?.total_distance ?? 0)

  /** true si les keyframes bruts sont disponibles (pré-calcul effectué). */
  const hasRawKeyframes = computed(() => rawKeyframes.value !== null)

  /** Overrides actifs uniquement (non désactivés), triés par début de plage. */
  const activeOverrides = computed(() =>
    overrides.value.overrides
      .filter(o => !o.disabled)
      .sort((a, b) => a.start_time - b.start_time),
  )

  // --- Actions : chargement / sauvegarde ---

  /**
   * Charge l'état d'édition d'une trace depuis le backend.
   *
   * Vérifie d'abord l'existence d'un cache de keyframes bruts (via
   * `has_raw_keyframes`). S'il existe, le charge ; sinon laisse
   * `rawKeyframes` à null pour que l'atelier déclenche le pré-calcul.
   * Charge toujours les overrides (vierges si première édition).
   *
   * @returns true si un cache de keyframes existait déjà, false sinon.
   */
  async function loadForTrace(traceId: string): Promise<boolean> {
    activeTraceId.value = traceId
    currentTime.value = 0
    isPlaying.value = false

    const cached = await invoke<boolean>('has_raw_keyframes', { traceId })

    if (cached) {
      rawKeyframes.value = await invoke<RawKeyframesFile>('get_raw_keyframes', { traceId })
      precomputeStatus.value = 'done'
    } else {
      rawKeyframes.value = null
      precomputeStatus.value = 'needed'
    }

    overrides.value = await invoke<MontageOverridesFile>('get_montage_overrides', { traceId })
    precomputeProgress.value = 0
    precomputeStep.value = ''
    return cached
  }

  /**
   * Persiste les keyframes bruts sur disque (appelé à la fin du pré-calcul).
   */
  async function saveRawKeyframes(file: RawKeyframesFile): Promise<void> {
    if (!activeTraceId.value) return
    await invoke('save_raw_keyframes', { traceId: activeTraceId.value, file })
    rawKeyframes.value = file
    precomputeStatus.value = 'done'
  }

  /**
   * Persiste les overrides de montage sur disque.
   * La refusion des keyframes finaux est déclenchée côté composable (watcher).
   */
  async function saveOverrides(): Promise<void> {
    if (!activeTraceId.value) return
    await invoke('save_montage_overrides', { traceId: activeTraceId.value, file: overrides.value })
  }

  // --- Actions : édition des overrides ---

  /** Ajoute un override et sauvegarde immédiatement. */
  async function addOverride(override: Override): Promise<void> {
    overrides.value.overrides.push(override)
    await saveOverrides()
  }

  /**
   * Met à jour un override existant (recherche par id) et sauvegarde.
   * Le patch est fusionné superficiellement ; les `params` sont remplacés
   * intégralement (cohérent avec l'UI qui édite tous les sliders d'un coup).
   */
  async function updateOverride(id: string, patch: Partial<Override>): Promise<void> {
    const idx = overrides.value.overrides.findIndex(o => o.id === id)
    if (idx === -1) return
    overrides.value.overrides[idx] = { ...overrides.value.overrides[idx], ...patch }
    await saveOverrides()
  }

  /** Supprime un override et sauvegarde. */
  async function removeOverride(id: string): Promise<void> {
    overrides.value.overrides = overrides.value.overrides.filter(o => o.id !== id)
    await saveOverrides()
  }

  /** Bascule l'activation d'un override (comparaison de rendu) et sauvegarde. */
  async function toggleOverride(id: string): Promise<void> {
    const ov = overrides.value.overrides.find(o => o.id === id)
    if (!ov) return
    ov.disabled = !ov.disabled
    await saveOverrides()
  }

  // --- Actions : lecture ---

  /** Place le curseur de lecture à l'instant donné (ms), borné à [0, total]. */
  function seek(time: number): void {
    const max = totalDuration.value
    currentTime.value = max > 0 ? Math.max(0, Math.min(time, max)) : 0
  }

  /** Démarre la lecture (la boucle RAF est pilotée par le composable). */
  function play(): void {
    if (!hasRawKeyframes.value) return
    // Repartir du début si on était à la fin.
    if (currentTime.value >= totalDuration.value) currentTime.value = 0
    isPlaying.value = true
  }

  /** Met en pause la lecture. */
  function pause(): void {
    isPlaying.value = false
  }

  /** Bascule lecture / pause. */
  function togglePlay(): void {
    if (isPlaying.value) pause()
    else play()
  }

  /** Réinitialise l'état (au démontage de la vue d'édition). */
  function reset(): void {
    activeTraceId.value = null
    rawKeyframes.value = null
    overrides.value = { overrides: [], messages: [], pois: [] }
    currentTime.value = 0
    isPlaying.value = false
    playbackSpeed.value = 1
    precomputeStatus.value = 'idle'
    precomputeProgress.value = 0
    precomputeStep.value = ''
  }

  // Exposition publique
  return {
    // État
    activeTraceId,
    rawKeyframes,
    overrides,
    currentTime,
    isPlaying,
    playbackSpeed,
    precomputeStatus,
    precomputeProgress,
    precomputeStep,
    // Getters
    totalDuration,
    totalDistance,
    hasRawKeyframes,
    activeOverrides,
    // Actions : chargement / sauvegarde
    loadForTrace,
    saveRawKeyframes,
    saveOverrides,
    // Actions : overrides
    addOverride,
    updateOverride,
    removeOverride,
    toggleOverride,
    // Actions : lecture
    seek,
    play,
    pause,
    togglePlay,
    // Divers
    reset,
  }
})
