// Store Pinia du module Multiride — détection des passages multiples.
//
// Miroir exact des structs Rust définies dans
// src-tauri/src/gpx_multiride/types.rs (elles-mêmes sérialisées en camelCase).
// Pattern Setup Store (conforme à docs/CONVENTIONS.md).
//
// Point de contrat : le store ne fait **aucun calcul métier**. La détection et
// les ajustements vivent dans les commandes Tauri
// (src-tauri/src/gpx_multiride/commands.rs) ; le store conserve l'état, expose
// des getters dérivés et orchestre les appels.
//
// Le fichier de description (`traces/{trace_id}/multiride.json`) est la seule
// persistance du module : il est écrit par la détection, relu pour restituer
// une détection précédente, et marqué `valide` à la validation. Le statut
// correspondant (`multiride_status`) vit dans le registre des traces, ce qui
// permet à la carte du circuit de connaître la barrière **sans lire de
// fichier**.
//
// Un segment ne porte qu'**un ajustement à la fois** — une fusion ou un faux
// positif, jamais les deux —, et cet ajustement s'annule individuellement
// (`undoSegment`). Les fusions, elles, s'enchaînent : un segment peut absorber
// son précédent puis le suivant, et la chaîne s'annule de la plus récente à la
// plus ancienne.

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'

// ─── Types primaires ──────────────────────────────────────────────────

/** Sens d'un passage, relatif au passage de référence de son segment. */
export type MultirideSens = 'reference' | 'aller' | 'retour'

/** Statuts du champ `multiride_status` du registre des traces. */
export type MultirideStatus = 'none' | 'pending' | 'validated'

// ─── Structures ───────────────────────────────────────────────────────

/** Coordonnées d'une borne de passage. */
export interface MultirideLatLon {
  lat: number
  lon: number
}

/** Paramètres de détection (namespace `Multiride.Detection`). */
export interface MultirideParams {
  toleranceM: number
  longueurMinM: number
  pasEchantillonnageM: number
  fusionReferencesM: number
}

/** Un emprunt d'un segment : portion de trace continue, qualifiée par un sens. */
export interface MultiridePassage {
  /** Numéro du segment (1-based) auquel le passage appartient. */
  segment: number
  /** Numéro du passage dans son segment (1-based). */
  passage: number
  sens: MultirideSens
  /** Segment marqué faux positif (exclu de l'export). */
  fauxPositif: boolean
  /** Index du point d'entrée dans les `<trkpt>` du GPX (1-based). */
  pointEntree: number
  /** Index du point de sortie (1-based). */
  pointSortie: number
  /** Distance cumulée depuis le départ de la trace (km). */
  kmEntree: number
  kmSortie: number
  longueurKm: number
  /** Le segment du passage a subi une fusion manuelle. */
  fusionne: boolean
  /**
   * Segment **approuvé** : un vrai passage multiple, rien à changer.
   *
   * Troisième état d'un segment, exclusif des deux autres, et comme eux un fait
   * de segment porté par chaque emprunt. C'est le geste le plus fréquent, et le
   * seul dont l'annulation ne demande aucune donnée.
   */
  valide: boolean
  /**
   * Emprunts des deux segments **tels qu'ils étaient avant la fusion** — de
   * quoi l'annuler.
   *
   * Présent sur les seuls emprunts d'un segment fusionné, et absent partout
   * ailleurs. Les emprunts enregistrés conservent leurs propres champs, y
   * compris un `avantFusion` antérieur : une chaîne de fusions s'annule donc
   * pas à pas, la plus récente d'abord.
   */
  avantFusion?: MultiridePassage[] | null
  entree: MultirideLatLon
  sortie: MultirideLatLon
}

/**
 * État complet d'une détection : état de travail de la vue et contenu du
 * fichier de description, dont il est la représentation interne.
 */
export interface MultirideArchive {
  version: number
  traceId: string
  source: string
  updatedAt: string
  /** L'utilisateur a validé la détection : la barrière est levée. */
  valide: boolean
  params: MultirideParams
  tracePointCount: number
  traceLengthKm: number
  /** Le pas d'échantillonnage a été relevé automatiquement (garde-fou). */
  pasPlafonne: boolean
  passages: MultiridePassage[]
}

/** Résultat de la commande `multiride_detect`. */
export interface MultirideDetectionResult {
  archive: MultirideArchive
  status: MultirideStatus
  durationMs: number
}

// ─── Aides pures ──────────────────────────────────────────────────────

/**
 * Numéros des segments dont **au moins un** emprunt satisfait `flag`, triés.
 *
 * Les marques d'ajustement sont des faits de segment portés par chaque emprunt
 * (comme côté Rust, `file::count_segments`) : c'est pourquoi on compte des
 * segments **distincts** et non des emprunts.
 */
function segmentNumbersWhere(
  passages: MultiridePassage[],
  flag: (passage: MultiridePassage) => boolean,
): number[] {
  return [...new Set(passages.filter(flag).map((p) => p.segment))].sort((a, b) => a - b)
}

// ─── Store ────────────────────────────────────────────────────────────

export const useMultirideStore = defineStore('multiride', () => {
  // ─── State ─────────────────────────────────────────────────────
  /** Trace dont la détection est chargée. */
  const currentTraceId = ref<string | null>(null)
  /** État de la détection courante (`null` tant qu'aucune n'est chargée). */
  const archive = ref<MultirideArchive | null>(null)
  /** Segment sélectionné dans la liste (`null` : aucun). */
  const selectedSegment = ref<number | null>(null)
  /** Durée de la dernière détection jouée (ms). */
  const analysisDurationMs = ref<number>(0)
  /** Une détection ou une relecture est en cours. */
  const loading = ref(false)

  // ─── Getters ───────────────────────────────────────────────────
  /** Passages détectés, tous segments confondus. */
  const passages = computed(() => archive.value?.passages ?? [])

  /** Numéros des segments détectés, dans l'ordre. */
  const segmentNumbers = computed(() =>
    [...new Set(passages.value.map((p) => p.segment))].sort((a, b) => a - b),
  )

  /** Nombre de segments détectés. */
  const segmentCount = computed(() => segmentNumbers.value.length)

  /**
   * Statut de l'état chargé (`null` sans détection) — miroir de
   * `MultirideArchive::status` côté Rust : **l'absence de passage prime sur la
   * validation**. `none` est un fait sur la trace (aucune portion répétée),
   * `validated` une décision de l'utilisateur sur une détection non vide.
   */
  const status = computed<MultirideStatus | null>(() => {
    if (!archive.value) return null
    if (!passages.value.length) return 'none'
    return archive.value.valide ? 'validated' : 'pending'
  })

  /** `true` quand l'édition caméra reste fermée par la détection. */
  const needsValidation = computed(() => status.value === 'pending')

  /**
   * `true` dès qu'un ajustement a été porté à la détection — un segment écarté
   * ou fusionné.
   *
   * C'est **la** condition du verrouillage des paramètres : ces deux gestes
   * changent le résultat, et une relance les effacerait. Une approbation, elle,
   * ne change rien au résultat — elle sera simplement à refaire sur la
   * détection relancée —, et ne verrouille donc pas les paramètres.
   */
  const hasAdjustments = computed(() =>
    passages.value.some((p) => p.fauxPositif || p.fusionne),
  )

  /** Segments marqués faux positifs. */
  const falsePositiveSegmentCount = computed(
    () => segmentNumbersWhere(passages.value, (p) => p.fauxPositif).length,
  )

  /**
   * Segments **jugés** : approuvés ou écartés.
   *
   * C'est le `x` du compteur d'avancement, et il compte des verdicts — pas des
   * réorganisations. Un segment fusionné n'a rien dit de sa justesse : c'est un
   * segment **neuf**, qui reste à juger, et il figure donc parmi les segments à
   * examiner tant qu'il n'a pas été approuvé ou écarté. L'union, et non la
   * somme : un segment ne porte qu'un verdict, mais un état exceptionnel
   * (fichier modifié à la main) ne doit pas faire mentir le décompte.
   */
  const treatedSegmentCount = computed(
    () => segmentNumbersWhere(passages.value, (p) => p.valide || p.fauxPositif).length,
  )

  /**
   * Segments **sans** ajustement : ce que le compteur de progression de la
   * barre compte à côté des traités, comme le `pending` des anomalies.
   * Borné à zéro — un décompte négatif ne voudrait rien dire.
   */
  const pendingSegmentCount = computed(() =>
    Math.max(0, segmentCount.value - treatedSegmentCount.value),
  )

  /**
   * Longueur cumulée des portions répétées (km) : somme des longueurs des
   * passages de **référence** des segments non marqués faux positif — chaque
   * portion répétée est ainsi comptée une fois.
   */
  const repeatedKm = computed(() =>
    passages.value
      .filter((p) => p.sens === 'reference' && !p.fauxPositif)
      .reduce((total, p) => total + p.longueurKm, 0),
  )

  // ─── Actions ───────────────────────────────────────────────────

  /** Passages d'un segment, triés par numéro de passage. */
  function segmentPassages(segment: number): MultiridePassage[] {
    return passages.value
      .filter((p) => p.segment === segment)
      .sort((a, b) => a.passage - b.passage)
  }

  /**
   * Emprunt de **référence** d'un segment (le premier), ou `null`.
   *
   * C'est lui qui porte la longueur et le début affichés en tête de segment :
   * la représentation d'un passage multiple se lit sur sa référence, les autres
   * emprunts n'étant que ses répétitions.
   */
  function referenceOf(segment: number): MultiridePassage | null {
    return segmentPassages(segment).find((p) => p.sens === 'reference') ?? null
  }

  /**
   * Emprunts du segment **précédent**, ou liste vide pour le premier segment.
   *
   * La fusion — et son refus quand un des deux camps est écarté — se décide sur
   * ces emprunts, que la barre d'action affiche en grisant le bouton.
   */
  function previousSegmentPassages(segment: number): MultiridePassage[] {
    return segment > 1 ? segmentPassages(segment - 1) : []
  }

  /** Sélectionne ou désélectionne un segment (mise en avant sur la carte). */
  function selectSegment(segment: number | null): void {
    selectedSegment.value = segment
  }

  /**
   * Joue la détection des passages multiples : lit le GPX de la trace, écrit le
   * fichier de description et pose le statut (`none` ou `pending`).
   *
   * Une relance **écrase** la détection précédente et ses ajustements : c'est
   * la contrepartie assumée d'un changement de paramètre — les segments
   * produits ne sont plus les mêmes, les marques qui les visaient n'ont plus
   * d'objet.
   */
  async function runDetection(traceId: string, params: MultirideParams): Promise<void> {
    loading.value = true
    try {
      const result = await invoke<MultirideDetectionResult>('multiride_detect', {
        traceId,
        params,
      })
      currentTraceId.value = traceId
      archive.value = result.archive
      analysisDurationMs.value = result.durationMs
      selectedSegment.value = null
    } finally {
      loading.value = false
    }
  }

  /**
   * Charge la détection persistée d'une trace.
   *
   * @returns `false` si aucun fichier exploitable n'existe — l'appelant
   * relance alors la détection, qui reste la source de vérité.
   */
  async function restore(traceId: string): Promise<boolean> {
    loading.value = true
    try {
      const loaded = await invoke<MultirideArchive | null>('multiride_load', { traceId })
      if (!loaded) return false

      currentTraceId.value = traceId
      archive.value = loaded
      // La durée d'analyse n'est pas persistée : elle ne vaut que pour la
      // détection qui vient d'être jouée.
      analysisDurationMs.value = 0
      selectedSegment.value = null
      return true
    } finally {
      loading.value = false
    }
  }

  /**
   * Fusionne un segment avec le précédent et réécrit le fichier de description.
   *
   * La règle est celle de la spécification : seuls des emprunts de **même sens**
   * fusionnent, et l'écart entre eux doit tenir dans un kilomètre — une décision
   * explicite de l'utilisateur, indépendante du réglage de fusion de la
   * détection.
   */
  async function mergeSegment(segment: number): Promise<void> {
    if (!currentTraceId.value || !archive.value) return
    archive.value = await invoke<MultirideArchive>('multiride_merge_segment', {
      traceId: currentTraceId.value,
      archive: archive.value,
      segment,
    })
    // La sélection suit la fusion : le segment absorbant porte le rang du
    // précédent.
    selectedSegment.value = segment > 1 ? segment - 1 : segment
  }

  /** Marque ou démarque un segment en faux positif, et réécrit le fichier. */
  async function toggleFp(segment: number): Promise<void> {
    if (!currentTraceId.value || !archive.value) return
    archive.value = await invoke<MultirideArchive>('multiride_toggle_fp', {
      traceId: currentTraceId.value,
      archive: archive.value,
      segment,
    })
  }

  /**
   * Approuve un segment tel qu'il a été détecté — un vrai passage multiple,
   * rien à changer — et réécrit le fichier de description.
   *
   * C'est le geste le plus fréquent de la vue, et le seul dont l'annulation ne
   * demande aucune donnée : l'approbation se retire comme elle s'est posée.
   */
  async function validateSegment(segment: number): Promise<void> {
    if (!currentTraceId.value || !archive.value) return
    archive.value = await invoke<MultirideArchive>('multiride_validate_segment', {
      traceId: currentTraceId.value,
      archive: archive.value,
      segment,
    })
  }

    /**
     * Annule l'état d'un segment et réécrit le fichier de description.
     *
     * Un segment ne portant qu'un état à la fois, la commande n'a pas à savoir
     * lequel elle défait : l'état enregistré le dit — approbation ou marqueur à
     * retirer, ou emprunts d'avant fusion à réinstaller, avec le rang des
     * segments que la fusion avait décalés.
     *
     * La sélection est conservée : le segment existe toujours après
     * l'annulation, c'est son état qui change.
     */
  async function undoSegment(segment: number): Promise<void> {
    if (!currentTraceId.value || !archive.value) return
    archive.value = await invoke<MultirideArchive>('multiride_undo_segment', {
      traceId: currentTraceId.value,
      archive: archive.value,
      segment,
    })
  }

  /**
   * Valide les passages détectés : marque l'état `valide`, réécrit le fichier
   * de description et lève la barrière de l'édition caméra.
   *
   * Les ajustements restent possibles après validation, et réécrivent le
   * fichier **sans** faire rebasculer le statut : ils n'ont pas d'incidence sur
   * l'édition caméra.
   */
  async function validate(): Promise<void> {
    if (!currentTraceId.value || !archive.value) return
    archive.value = await invoke<MultirideArchive>('multiride_validate', {
      traceId: currentTraceId.value,
      archive: archive.value,
    })
  }

  /** Réinitialise intégralement le store (sortie de la vue). */
  function reset(): void {
    currentTraceId.value = null
    archive.value = null
    selectedSegment.value = null
    analysisDurationMs.value = 0
    loading.value = false
  }

  return {
    // State
    currentTraceId,
    archive,
    selectedSegment,
    analysisDurationMs,
    loading,
    // Getters
    passages,
    segmentNumbers,
    segmentCount,
    status,
    needsValidation,
    hasAdjustments,
    falsePositiveSegmentCount,
    treatedSegmentCount,
    pendingSegmentCount,
    repeatedKm,
    // Actions
    segmentPassages,
    referenceOf,
    previousSegmentPassages,
    selectSegment,
    runDetection,
    restore,
    validateSegment,
    mergeSegment,
    toggleFp,
    undoSegment,
    validate,
    reset,
  }
})
