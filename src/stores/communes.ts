/**
 * Store Pinia de la détection des communes traversées.
 *
 * Il orchestre la fonctionnalité : **une seule analyse à la fois**, dans une
 * file d'attente séquentielle. Chaque trace suit le même chemin — Overpass
 * (`useCommunesOverpass`, le réseau restant côté WebView) puis `communes_detect`
 * (géométrie, écriture du fichier et drapeau du registre, côté Rust).
 *
 * Deux points d'entrée :
 * - `request(traceId)` : un circuit précis — import d'une trace déjà propre, fin
 *   d'audit, ou clic sur la carte du circuit. La trace passe **en tête** de file :
 *   l'analyse en cours se termine, puis celle-ci est traitée ;
 * - `startCatchUp()` : au lancement, toutes les traces **déjà nettoyées** dont la
 *   détection n'a pas abouti. Rien à faire → aucune tâche ne démarre.
 *
 * L'état exposé alimente l'icône de la carte du circuit (`Circuit.vue`) :
 * `isRunning` (tâche en cours) et `isPending(traceId)` (circuit en cours de
 * traitement ou en file).
 */

import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useCommunesOverpass } from '../composables/useCommunesOverpass'
// Import croisé avec `traces.ts` : chaque store n'appelle l'autre qu'à
// l'intérieur de ses actions, jamais au chargement du module.
import { useTracesStore } from './traces'
import type { TraceMetadata } from './traces'
import { useUiStore } from './ui'

/**
 * Un passage dans une commune. Miroir exact de la struct Rust
 * `gpx_communes::types::CommunePassage` (snake_case) — c'est aussi le format du
 * fichier `traces/{trace_id}/communes.json`.
 */
export interface CommunePassage {
  /** Nom de la commune (tag `name` de la relation OSM). */
  commune: string
  /** Numéro de ce passage dans la commune (1-based, ordre chronologique). */
  passage: number
  /** Nombre total de passages dans cette commune. */
  nb_passages_total: number
  /** Kilomètre d'entrée dans la commune, depuis le départ de la trace. */
  km_entree: number
  /** Kilomètre de sortie de la commune. */
  km_sortie: number
  /** Longueur du passage (`km_sortie - km_entree`). */
  longueur_km: number
}

/** Pause entre deux requêtes Overpass (politesse vis-à-vis de l'API publique). */
const INTER_TRACE_DELAY_MS = 1500

/** Attente entre deux analyses de la file. */
function pause(ms: number): Promise<void> {
  return new Promise(resolve => window.setTimeout(resolve, ms))
}

export const useCommunesStore = defineStore('communes', () => {
  const ui = useUiStore()
  const overpass = useCommunesOverpass()

  // État

  /** Identifiants en attente : le premier de la file est le prochain traité. */
  const queue = ref<string[]>([])
  /** Trace en cours d'analyse, ou `null`. */
  const currentTraceId = ref<string | null>(null)
  /** Vrai tant que la tâche traite la file. */
  const isRunning = ref(false)

  /**
   * Traces en échec **dans cette session** : elles restent éligibles au regard
   * du registre, mais ne sont pas retentées en boucle par la tâche de fond — la
   * reprise a lieu au lancement suivant, ou au clic sur le circuit. Une demande
   * explicite réarme la trace.
   */
  const failed = new Set<string>()

  // Getters

  /** Une trace est éligible si elle est nettoyée et sans détection aboutie. */
  function isEligible(trace: TraceMetadata): boolean {
    return trace.audit_status === 'clean' && !trace.communes_completed
  }

  /** Un traitement est-il en cours (ou en file) pour ce circuit ? */
  function isPending(traceId: string): boolean {
    return currentTraceId.value === traceId || queue.value.includes(traceId)
  }

  // Actions

  /**
   * Registre relu à la source (`get_traces`) : la détection écrit
   * `communes_completed` côté backend, et une trace peut avoir été nettoyée,
   * supprimée ou ré-auditée entre-temps. Lecture seule.
   */
  async function freshTraces(): Promise<TraceMetadata[]> {
    try {
      return await invoke<TraceMetadata[]>('get_traces')
    } catch (error) {
      console.error('[communes] Registre illisible :', error)
      return []
    }
  }

  /** Détecte les communes d'une trace : Overpass, puis analyse et écriture. */
  async function processOne(traceId: string): Promise<number> {
    const osm = await overpass.fetchCommunes(traceId)
    const passages = await invoke<CommunePassage[]>('communes_detect', { traceId, osm })
    return passages.length
  }

  /**
   * Tâche de fond : vide la file, une trace à la fois.
   *
   * L'éligibilité est revérifiée **avant chaque trace**, registre relu à la
   * source : une trace nettoyée pendant la tâche est prise en compte, une trace
   * déjà traitée ou repassée à `needs_review` est ignorée.
   */
  async function run(): Promise<void> {
    if (isRunning.value) return
    isRunning.value = true

    const failures: string[] = []
    let done = 0
    try {
      while (queue.value.length > 0) {
        const traceId = queue.value.shift()
        if (!traceId || failed.has(traceId)) continue
        currentTraceId.value = traceId

        const trace = (await freshTraces()).find(t => t.id === traceId)
        if (!trace || !isEligible(trace)) continue

        try {
          const passages = await processOne(traceId)
          done += 1
          // La commande a écrit le drapeau côté backend : l'entrée en mémoire
          // doit le refléter sans relire le registre — la carte du circuit s'y
          // adosse, et un circuit détecté repasse au vert sans attendre la fin
          // du lot.
          useTracesStore().markCommunesCompleted(traceId)
          console.log(`[communes] ${trace.name} : ${passages} passage(s).`)
        } catch (error) {
          const reason = error instanceof Error ? error.message : String(error)
          failed.add(traceId)
          failures.push(`${trace.name} (${reason})`)
          console.warn(`[communes] Détection impossible pour « ${trace.name} » : ${reason}`)
        }

        if (queue.value.length > 0) await pause(INTER_TRACE_DELAY_MS)
      }
    } finally {
      isRunning.value = false
      currentTraceId.value = null
      report(done, failures)
    }
  }

  /**
   * Bilan de fin de tâche : **une seule** notification, jamais une par circuit —
   * un rattrapage au lancement peut en enchaîner des dizaines.
   */
  function report(done: number, failures: string[]): void {
    if (failures.length === 0) {
      if (done > 0) {
        ui.showSuccess(
          done === 1
            ? 'Communes détectées pour 1 circuit.'
            : `Communes détectées pour ${done} circuits.`,
        )
      }
      return
    }
    const first = failures.length > 1 ? `${failures[0]} +${failures.length - 1}` : failures[0]
    ui.showWarning(`Détection des communes incomplète : ${done} réussite(s), échec sur ${first}.`)
  }

  /**
   * Demande la détection pour un circuit.
   *
   * La trace passe en **tête** de file : la tâche en cours termine l'analyse
   * qu'elle a commencée, puis traite celle-ci — jamais deux requêtes Overpass en
   * parallèle, ni deux écritures concurrentes. Sans tâche en cours, elle démarre
   * immédiatement.
   */
  function request(traceId: string): void {
    failed.delete(traceId)
    if (currentTraceId.value === traceId || queue.value.includes(traceId)) return
    queue.value.unshift(traceId)
    void run()
  }

  /**
   * Rattrapage au lancement : met en file **toutes** les traces déjà nettoyées
   * dont la détection n'a pas abouti, puis attend la fin de la tâche.
   *
   * Rien à faire → aucune tâche ne démarre.
   */
  async function startCatchUp(): Promise<void> {
    const pending = (await freshTraces()).filter(isEligible).map(t => t.id)
    if (pending.length === 0) return

    for (const traceId of pending) {
      if (!queue.value.includes(traceId)) queue.value.push(traceId)
    }
    await run()
  }

  return {
    // État
    currentTraceId,
    isRunning,
    // Getters
    isPending,
    // Actions
    request,
    startCatchUp,
  }
})
