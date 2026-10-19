import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import {
  climbToEdit,
  constrainClimbBound,
  type ClimbHandle,
} from '../utils/altimetrieClimbs'

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

  /**
   * Reconstruit l'analyse avec des montées éditées, sans écrire le fichier.
   *
   * Le backend reprend les paramètres et l'horodatage du fichier existant. En
   * cas d'échec, l'état précédent est restauré : le widget ne conserve pas une
   * borne que le backend a refusée.
   */
  async function preview(
    traceId: string,
    climbs: AltimetrieClimbEdit[],
  ): Promise<void> {
    const snapshot = file.value
    try {
      file.value = await invoke<AltimetrieFile>('altimetrie_preview', {
        traceId,
        climbs,
      })
      dirty.value = true
    } catch (error) {
      file.value = snapshot
      throw error
    }
  }

  /**
   * Déplace une borne de montée et reconstruit l'aperçu.
   *
   * Les contraintes de voisinage et de longueur minimale sont appliquées ici,
   * puis revalidées par le backend. Un déplacement nul ne déclenche pas
   * d'appel IPC ni ne marque l'état modifié.
   */
  async function updateClimbBound(
    traceId: string,
    climbId: number,
    handle: ClimbHandle,
    km: number,
  ): Promise<void> {
    const current = file.value
    if (!current) return

    const climb = current.climbs.find(entry => entry.id === climbId)
    if (!climb) return

    const lastProfilePoint = current.profile[current.profile.length - 1]
    const totalKm = lastProfilePoint
      ? lastProfilePoint.d / 1000
      : current.trace.totalDistanceM / 1000
    const constrained = constrainClimbBound(
      current.climbs,
      climbId,
      handle,
      km,
      totalKm,
    )
    const original = handle === 'valley' ? climb.startKm : climb.endKm
    if (Math.abs(constrained - original) < 1e-4) return

    const edits = current.climbs.map(entry => {
      const edit = climbToEdit(entry)
      if (entry.id === climbId) {
        if (handle === 'valley') edit.startKm = constrained
        else edit.endKm = constrained
      }
      return edit
    })

    await preview(traceId, edits)
  }

  /** Enregistre les montées courantes ; le backend met à jour `updatedAt`. */
  async function save(traceId: string): Promise<void> {
    const current = file.value
    if (!current) return

    saving.value = true
    try {
      const climbs = current.climbs.map(climbToEdit)
      file.value = await invoke<AltimetrieFile>('altimetrie_save', {
        traceId,
        climbs,
      })
      dirty.value = false
    } finally {
      saving.value = false
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
    preview,
    updateClimbBound,
    save,
    selectClimb,
    reset,
  }
})
