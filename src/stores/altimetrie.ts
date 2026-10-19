import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'

/** Paramètres du pipeline, miroir de `AltimetrieParams` côté Rust. */
export interface AltimetrieParams {
  pasEchantillonnage: number
  correctionPlateaux: number
  fenetreAntiPic: number
  seuilAntiPic: number
  fenetreAltitude: number
  fenetrePente: number
  sigmaGaussien: number
  fenetreMediane: number
  lissageRenforce: boolean
  seuilMontee: number
  ecartFusion: number
  rechercheExtrema: number
  fenetreCalcul: number
  longueurMin: number
}

/** Contexte de la trace dans le fichier. */
export interface AltimetrieTrace {
  totalDistanceM: number
  totalDistanceSampledM: number
  trailingGapM: number
  stepM: number
  totalPoints: number
}

/** Point du profil affiché. */
export interface AltimetrieProfilePoint {
  d: number
  ele: number
  lat: number
  lon: number
}

/** Segment de pente. */
export interface AltimetrieSegment {
  index: number
  startKm: number
  endKm: number
  startProgress: number
  endProgress: number
  categoryId: string
  avgSlopePct: number
  dPlusM: number
}

/** Sous-montée d'une montée fusionnée. */
export interface AltimetrieMergedPart {
  partIndex: number
  startKm: number
  endKm: number
  startProgress: number
  endProgress: number
  dPlusM: number
  avgSlopePct: number
  altStartM: number
  altEndM: number
}

/** Montée détectée ou créée manuellement. */
export interface AltimetrieClimb {
  id: number
  index: number
  startKm: number
  endKm: number
  startProgress: number
  endProgress: number
  dPlusM: number
  avgSlopePct: number
  altStartM: number
  altEndM: number
  mergedCount: number
  fallback: boolean
  isMerged: boolean
  isSeparated: boolean
  groupId?: number | null
  mergedParts?: AltimetrieMergedPart[] | null
}

/** Définition d'une catégorie de pente dans la légende. */
export interface AltimetrieCategoryDef {
  id: string
  label: string
  minPct: number | null
  maxPct: number | null
}

/** Légende du fichier. */
export interface AltimetrieLegend {
  note: string
  fenetreCalcul: number
  longueurMin: number
  categories: AltimetrieCategoryDef[]
}

/** Contenu complet de `traces/{trace_id}/altimetrie.json`. */
export interface AltimetrieFile {
  version: number
  traceId: string
  source: string
  updatedAt: string
  params: AltimetrieParams
  trace: AltimetrieTrace
  profile: AltimetrieProfilePoint[]
  segments: AltimetrieSegment[]
  climbs: AltimetrieClimb[]
  legend: AltimetrieLegend
}

/** Sous-montée éditable. */
export interface AltimetrieMergedPartEdit {
  startKm: number
  endKm: number
}

/** Forme éditable d'une montée, miroir de `AltimetrieClimbEdit` côté Rust. */
export interface AltimetrieClimbEdit {
  id: number
  startKm: number
  endKm: number
  isMerged: boolean
  isSeparated: boolean
  groupId: number | null
  mergedParts: AltimetrieMergedPartEdit[] | null
  fallback: boolean
}

export const useAltimetrieStore = defineStore('altimetrie', () => {
  // ─── État ──────────────────────────────────────────────────────────
  const file = ref<AltimetrieFile | null>(null)
  const loading = ref(false)
  const saving = ref(false)
  /** Des montées ont été éditées et ne sont pas encore enregistrées. */
  const dirty = ref(false)
  const selectedClimbId = ref<number | null>(null)

  // ─── Getters ───────────────────────────────────────────────────────
  const climbs = computed(() => file.value?.climbs ?? [])
  const selectedClimb = computed(
    () => climbs.value.find(climb => climb.id === selectedClimbId.value) ?? null,
  )

  // ─── Actions ───────────────────────────────────────────────────────
  /**
   * Charge l'analyse existante ou la génère si le fichier est absent.
   *
   * C'est le point d'entrée de la vue : les traces importées avant la
   * fonctionnalité, ou dont la génération automatique a échoué, sont couvertes
   * à la première ouverture.
   */
  async function loadOrDetect(traceId: string, params: AltimetrieParams): Promise<void> {
    loading.value = true
    try {
      const existing = await invoke<AltimetrieFile | null>('altimetrie_load', { traceId })
      file.value =
        existing ?? (await invoke<AltimetrieFile>('altimetrie_detect', { traceId, params }))
      dirty.value = false
      selectedClimbId.value = null
    } finally {
      loading.value = false
    }
  }

  /** Rejoue la détection avec les paramètres fournis (recalcul explicite). */
  async function detect(traceId: string, params: AltimetrieParams): Promise<void> {
    loading.value = true
    try {
      file.value = await invoke<AltimetrieFile>('altimetrie_detect', { traceId, params })
      dirty.value = false
      selectedClimbId.value = null
    } finally {
      loading.value = false
    }
  }

  /** Sélectionne une montée (ou désélectionne avec `null`). */
  function selectClimb(id: number | null): void {
    selectedClimbId.value = id
  }

  /** Réinitialise l'état à la sortie de la vue. */
  function reset(): void {
    file.value = null
    loading.value = false
    saving.value = false
    dirty.value = false
    selectedClimbId.value = null
  }

  return {
    // État
    file,
    loading,
    saving,
    dirty,
    selectedClimbId,
    // Getters
    climbs,
    selectedClimb,
    // Actions
    loadOrDetect,
    detect,
    selectClimb,
    reset,
  }
})
