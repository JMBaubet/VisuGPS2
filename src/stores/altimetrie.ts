import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import {
  applyClimbBounds,
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

/** Bornes d'une montée en cours de glissement, avant validation backend. */
interface BoundsDraft {
  climbId: number
  startKm: number
  endKm: number
}

export const useAltimetrieStore = defineStore('altimetrie', () => {
  // ─── État ──────────────────────────────────────────────────────────
  const file = ref<AltimetrieFile | null>(null)
  const loading = ref(false)
  const saving = ref(false)
  /** Des montées ont été éditées et ne sont pas encore enregistrées. */
  const dirty = ref(false)
  /**
   * L'opérateur a validé l'analyse : une sauvegarde utilisateur a été effectuée.
   *
   * Distinct de `dirty` : une analyse générée automatiquement n'est pas
   * validée, même sans modification en attente. Le drapeau persiste dans le
   * registre des traces (`altimetrie_validated`) et pilote le libellé du bouton
   * de la toolbar — « Valider » tant qu'aucune sauvegarde n'a eu lieu,
   * « Sauvegarder » ensuite.
   */
  const validated = ref(false)
  const selectedClimbId = ref<number | null>(null)
  /** Aperçu local d'une borne déplacée, affiché sans attendre le backend. */
  const boundsDraft = ref<BoundsDraft | null>(null)

  // ─── Getters ───────────────────────────────────────────────────────
  /**
   * Fichier affiché : l'état persisté, avec la borne en cours de glissement
   * appliquée. La carte, le widget et les cartes de montée lisent cette
   * projection pour réagir en temps réel, sans appel IPC pendant le drag.
   */
  const displayFile = computed<AltimetrieFile | null>(() => {
    const current = file.value
    const draft = boundsDraft.value
    if (!current || !draft) return current

    return {
      ...current,
      climbs: current.climbs.map(climb =>
        climb.id === draft.climbId
          ? applyClimbBounds(
              climb,
              draft.startKm,
              draft.endKm,
              current.profile,
              current.trace.totalDistanceM,
            )
          : climb,
      ),
    }
  })
  const climbs = computed(() => displayFile.value?.climbs ?? [])
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
  async function loadOrDetect(
    traceId: string,
    params: AltimetrieParams,
    persistedValidated = false,
  ): Promise<void> {
    loading.value = true
    try {
      const existing = await invoke<AltimetrieFile | null>('altimetrie_load', { traceId })
      if (existing) {
        file.value = existing
        // Un fichier existant conserve l'état de validation persisté.
        validated.value = persistedValidated
      } else {
        // La détection produit une analyse neuve : le backend repose le
        // drapeau du registre, la validation appartient à l'opérateur.
        file.value = await invoke<AltimetrieFile>('altimetrie_detect', { traceId, params })
        validated.value = false
      }
      dirty.value = false
      selectedClimbId.value = null
      boundsDraft.value = null
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
      // Le backend repose le drapeau du registre : l'analyse recalculée n'a pas
      // été relue par l'opérateur.
      validated.value = false
      selectedClimbId.value = null
      boundsDraft.value = null
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
    } finally {
      boundsDraft.value = null
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
    if (!current) {
      boundsDraft.value = null
      return
    }

    const climb = current.climbs.find(entry => entry.id === climbId)
    if (!climb) {
      boundsDraft.value = null
      return
    }

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
    if (Math.abs(constrained - original) < 1e-4) {
      boundsDraft.value = null
      return
    }

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

  /**
   * Applique localement une borne déplacée, sans appel backend.
   *
   * Appelée à chaque déplacement du pointeur : la carte et les cartes de
   * montée suivent immédiatement, avec les métriques recalculées depuis le
   * profil courant. Le backend reprend la main au relâchement.
   */
  function draftClimbBound(
    climbId: number,
    handle: ClimbHandle,
    km: number,
  ): void {
    const current = file.value
    if (!current) return

    const climb = current.climbs.find(entry => entry.id === climbId)
    if (!climb) return

    const lastPoint = current.profile[current.profile.length - 1]
    const totalKm = lastPoint
      ? lastPoint.d / 1000
      : current.trace.totalDistanceM / 1000
    const constrained = constrainClimbBound(
      current.climbs,
      climbId,
      handle,
      km,
      totalKm,
    )

    boundsDraft.value = {
      climbId,
      startKm: handle === 'valley' ? constrained : climb.startKm,
      endKm: handle === 'summit' ? constrained : climb.endKm,
    }
  }

  /** Abandonne l'aperçu local (relâchement sans modification, annulation). */
  function clearClimbDraft(): void {
    boundsDraft.value = null
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
      // `altimetrie_save` pose le drapeau dans le registre : c'est la
      // validation de l'opérateur, qu'elle passe par « Valider » ou
      // « Sauvegarder ».
      validated.value = true
    } finally {
      saving.value = false
    }
  }

  /** Prochain identifiant de montée disponible. */
  function nextClimbId(climbs: AltimetrieClimb[]): number {
    return climbs.reduce((max, climb) => Math.max(max, climb.id), 0) + 1
  }

  /** Prochain identifiant de groupe séparé disponible. */
  function nextGroupId(climbs: AltimetrieClimb[]): number {
    return (
      climbs.reduce((max, climb) => Math.max(max, climb.groupId ?? 0), 0) + 1
    )
  }

  /**
   * Supprime une montée.
   *
   * Un membre d'un groupe séparé est retiré seul ; si le groupe tombe sous deux
   * membres, le dernier membre redevient une montée simple.
   */
  async function deleteClimb(
    traceId: string,
    climbId: number,
  ): Promise<void> {
    const current = file.value
    if (!current) return

    const target = current.climbs.find(climb => climb.id === climbId)
    if (!target) return

    const remaining = current.climbs
      .filter(climb => climb.id !== climbId)
      .map(climbToEdit)

    if (target.groupId != null) {
      const siblings = remaining.filter(
        climb => climb.groupId === target.groupId,
      )
      if (siblings.length < 2) {
        for (const sibling of siblings) {
          sibling.groupId = null
          sibling.isSeparated = false
        }
      }
    }

    await preview(traceId, remaining)
    if (selectedClimbId.value === climbId) {
      selectedClimbId.value = null
    }
  }

  /**
   * Sépare une montée fusionnée en ses sous-montées.
   *
   * Les membres partagent un nouveau `groupId` et portent `isSeparated` ;
   * leurs bornes sont celles des `mergedParts` conservées par le backend.
   */
  async function splitClimb(
    traceId: string,
    climbId: number,
  ): Promise<void> {
    const current = file.value
    if (!current) return

    const target = current.climbs.find(climb => climb.id === climbId)
    if (!target || !target.isMerged || !target.mergedParts) return

    const parts = target.mergedParts.filter(
      part => part.endKm > part.startKm,
    )
    if (parts.length < 2) return

    const groupId = nextGroupId(current.climbs)
    let nextId = nextClimbId(current.climbs)
    const members: AltimetrieClimbEdit[] = parts.map(part => ({
      id: nextId++,
      startKm: part.startKm,
      endKm: part.endKm,
      isMerged: false,
      isSeparated: true,
      groupId,
      mergedParts: null,
      fallback: false,
    }))

    const edits = current.climbs
      .filter(climb => climb.id !== climbId)
      .map(climbToEdit)
    edits.push(...members)

    await preview(traceId, edits)
    selectedClimbId.value = members[0].id
  }

  /**
   * Refusionne les membres d'un groupe séparé en une montée fusionnée.
   *
   * L'identifiant du premier membre est conservé ; les bornes courantes de
   * chaque membre deviennent les `mergedParts` de l'entrée fusionnée.
   */
  async function mergeClimb(
    traceId: string,
    climbId: number,
  ): Promise<void> {
    const current = file.value
    if (!current) return

    const target = current.climbs.find(climb => climb.id === climbId)
    if (!target || !target.isSeparated || target.groupId == null) return

    const groupId = target.groupId
    const members = current.climbs
      .filter(climb => climb.groupId === groupId)
      .sort((a, b) => a.startKm - b.startKm)
    if (members.length < 2) return

    const mergedId = members[0].id
    const merged: AltimetrieClimbEdit = {
      id: mergedId,
      startKm: members[0].startKm,
      endKm: members[members.length - 1].endKm,
      isMerged: true,
      isSeparated: false,
      groupId: null,
      mergedParts: members.map(member => ({
        startKm: member.startKm,
        endKm: member.endKm,
      })),
      fallback: false,
    }

    const edits = current.climbs
      .filter(climb => climb.groupId !== groupId)
      .map(climbToEdit)
    edits.push(merged)

    await preview(traceId, edits)
    selectedClimbId.value = mergedId
  }

  /** Crée une montée manuelle entre deux kilomètres. */
  async function createClimb(
    traceId: string,
    startKm: number,
    endKm: number,
  ): Promise<void> {
    const current = file.value
    if (!current) return

    const id = nextClimbId(current.climbs)
    const edit: AltimetrieClimbEdit = {
      id,
      startKm,
      endKm,
      isMerged: false,
      isSeparated: false,
      groupId: null,
      mergedParts: null,
      fallback: false,
    }

    const edits = current.climbs.map(climbToEdit)
    edits.push(edit)

    await preview(traceId, edits)
    selectedClimbId.value = id
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
    validated.value = false
    selectedClimbId.value = null
    boundsDraft.value = null
  }

  return {
    // État
    file,
    loading,
    saving,
    dirty,
    validated,
    selectedClimbId,
    // Getters
    displayFile,
    climbs,
    selectedClimb,
    // Actions
    loadOrDetect,
    detect,
    preview,
    updateClimbBound,
    draftClimbBound,
    clearClimbDraft,
    save,
    deleteClimb,
    splitClimb,
    mergeClimb,
    createClimb,
    selectClimb,
    reset,
  }
})
