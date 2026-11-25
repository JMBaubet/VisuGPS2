/**
 * Store Pinia de la détection des communes traversées.
 *
 * Il orchestre la fonctionnalité : **une seule analyse à la fois**, dans une
 * file d'attente séquentielle. La détection est faite par le **serveur hermes**
 * (`POST /communes/detect`, via la commande `communes_detect_hermes`) sur le
 * dataset servi. Le store vérifie d'abord que le service répond et que les
 * artefacts communes sont préparés (`hermesStore.ensureReady`), puis laisse le
 * backend relire la trace, appeler le serveur et écrire le fichier.
 *
 * Trois points d'entrée :
 * - `request(traceId)` : un circuit précis — import d'une trace déjà propre, fin
 *   d'audit, ou clic sur la carte du circuit. La trace passe **en tête** de
 *   file : l'analyse en cours se termine, puis celle-ci est traitée ;
 * - `request(traceId, { force: true })` : **recalcul** demandé depuis la fenêtre
 *   de consultation. Un statut `complete` ou `partial` ne bloque pas, et le
 *   fichier précédent reste en place jusqu'au succès : un échec ne détruit pas
 *   le résultat existant ;
 * - `startCatchUp()` : au lancement, toutes les traces **déjà nettoyées** dont la
 *   détection n'a pas abouti. Rien à faire → aucune tâche ne démarre. Appelée
 *   aussi au retour du service hermes, le serveur dormant au lancement.
 *
 * L'état exposé alimente l'icône de la carte du circuit (`Circuit.vue`) :
 * `isRunning` (tâche en cours) et `isPending(traceId)` (circuit en cours de
 * traitement ou en file). `outsideCoverageDataset(traceId)` porte, pour la
 * session, le dataset dont la trace est sortie à la dernière détection — le
 * fichier, lui, ne conserve que les passages.
 */

import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { parseHermesError } from '../utils/hermes'
import { useHermesStore } from './hermes'
// Import croisé avec `traces.ts` : chaque store n'appelle l'autre qu'à
// l'intérieur de ses actions, jamais au chargement du module.
import { useTracesStore } from './traces'
import type { CommunesStatus, TraceMetadata } from './traces'
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
  /**
   * Deux passages de communes différentes se chevauchent de plus de 1 m : la
   * frontière OSM est imprécise à cet endroit. `false` pour les fichiers
   * écrits avant la détection par hermes.
   */
  ambiguous: boolean
}

/**
 * Retour de `communes_detect_hermes` (camelCase) : les passages écrits, le
 * dataset utilisé, le verdict de couverture et l'état posé au registre.
 */
export interface CommuneDetection {
  passages: CommunePassage[]
  datasetId: string
  outsideCoverage: boolean
  /** `"complete"` si toute la trace est couverte, `"partial"` sinon. */
  status: CommunesStatus
}

/** Un élément de la file : `force` court-circuite une détection existante. */
interface QueueItem {
  traceId: string
  force: boolean
}

/**
 * Origine d'un lot : un clic parle, le rattrapage du lancement se tait (le
 * serveur dort, et le chip Hermes dit déjà pourquoi).
 */
type RunReason = 'request' | 'catch-up'

export const useCommunesStore = defineStore('communes', () => {
  const ui = useUiStore()
  const hermes = useHermesStore()

  // État

  /** Traces en attente : la première de la file est la prochaine traitée. */
  const queue = ref<QueueItem[]>([])
  /** Trace en cours d'analyse, ou `null`. */
  const currentTraceId = ref<string | null>(null)
  /** Vrai tant que la tâche traite la file. */
  const isRunning = ref(false)
  /**
   * Traces dont la dernière détection est sortie de la couverture du dataset,
   * avec le dataset en question. État de **session** : la fenêtre de
   * consultation s'en sert pour distinguer « hors zone » de « aucune commune ».
   */
  const outsideCoverage = ref<Record<string, string>>({})

  /**
   * Traces en échec **dans cette session** : elles restent éligibles au regard
   * du registre, mais ne sont pas retentées en boucle par la tâche de fond — la
   * reprise a lieu au lancement suivant, ou au clic sur le circuit. Une demande
   * explicite réarme la trace.
   */
  const failed = new Set<string>()

  // Getters

  /**
   * Une trace est éligible au rattrapage automatique si elle est nettoyée et
   * **jamais détectée**. Une détection partielle n'est pas reprise d'office :
   * l'icône jaune et la fenêtre de consultation proposent le recalcul manuel.
   */
  function isEligible(trace: TraceMetadata): boolean {
    return trace.audit_status === 'clean' && trace.communes_status === 'none'
  }

  /** Un traitement est-il en cours (ou en file) pour ce circuit ? */
  function isPending(traceId: string): boolean {
    return (
      currentTraceId.value === traceId ||
      queue.value.some(item => item.traceId === traceId)
    )
  }

  /** Dataset dont la trace est sortie, ou `null`. */
  function outsideCoverageDataset(traceId: string): string | null {
    return outsideCoverage.value[traceId] ?? null
  }

  // Actions

  /**
   * Registre relu à la source (`get_traces`) : la détection écrit
   * le statut des communes côté backend, et une trace peut avoir été nettoyée,
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

  /**
   * Le service est-il prêt pour le lot ? Une seule vérification par lot : le
   * dataset servi, ORS et l'artefact `communes.geojson` sont contrôlés avant la
   * première trace, puis le backend reste seul juge (il relit `/serving`).
   */
  async function readyToDetect(reason: RunReason): Promise<boolean> {
    const readiness = await hermes.ensureReady('communes')
    if (readiness.ok) return true

    // Au lancement, le serveur est en veille : l'échec est attendu, et le chip
    // Hermes en porte déjà l'explication. Seules les causes **durables**
    // (dataset absent, artefacts non préparés) méritent une notification.
    const durable =
      readiness.code !== 'HERMES_OFFLINE' && readiness.code !== 'HERMES_TIMEOUT'
    if (reason === 'request' || durable) {
      ui.showWarning(
        readiness.message ?? 'Détection des communes indisponible pour le moment.',
      )
    }
    return false
  }

  /** Détecte les communes d'une trace via hermes, et retient la couverture. */
  async function processOne(traceId: string): Promise<CommuneDetection> {
    const detection = await invoke<CommuneDetection>('communes_detect_hermes', {
      traceId,
    })
    if (detection.outsideCoverage) {
      outsideCoverage.value[traceId] = detection.datasetId
    } else {
      delete outsideCoverage.value[traceId]
    }
    return detection
  }

  /**
   * Relit la description des communes d'une trace, pour la consultation.
   *
   * `null` quand il n'y a rien à restituer (fichier absent ou illisible) : un
   * **tableau vide** est un résultat — la trace ne traverse aucune commune
   * connue — et non une absence.
   */
  async function loadPassages(traceId: string): Promise<CommunePassage[] | null> {
    try {
      return await invoke<CommunePassage[] | null>('communes_load', { traceId })
    } catch (error) {
      console.error(`[communes] Description illisible pour ${traceId} :`, error)
      return null
    }
  }

  /**
   * Réinitialise la détection d'une trace — outil de **diagnostic**, exposé en
   * mode debug depuis la fenêtre de consultation.
   *
   * La description est supprimée et le drapeau reposé : la trace redevient « à
   * détecter », reprise au prochain lancement ou immédiatement au clic sur sa
   * carte.
   */
  async function resetDetection(traceId: string): Promise<void> {
    await invoke('communes_reset', { traceId })
    failed.delete(traceId)
    delete outsideCoverage.value[traceId]
    useTracesStore().setCommunesStatus(traceId, 'none')
    ui.showInfo('Détection des communes réinitialisée : le circuit peut être relancé.')
  }

  /**
   * Tâche de fond : vide la file, une trace à la fois.
   *
   * La préparation du serveur est vérifiée **une fois par lot** ; l'éligibilité,
   * elle, est revérifiée **avant chaque trace**, registre relu à la source : une
   * trace nettoyée pendant la tâche est prise en compte, une trace déjà traitée
   * ou repassée à `needs_review` est ignorée.
   */
  async function run(reason: RunReason): Promise<void> {
    if (isRunning.value || queue.value.length === 0) return
    isRunning.value = true

    // Le service n'est pas prêt : la file est vidée pour que les icônes
    // repassent au rouge, et les traces restent éligibles (aucun drapeau posé).
    // Leurs demandes seront rejouées au retour du service ou au clic suivant.
    // Une défaillance inattendue de la préparation ne doit pas laisser
    // `isRunning` levé — la tâche serait alors bloquée pour la session.
    let ready = false
    try {
      ready = await readyToDetect(reason)
    } catch (error) {
      console.error('[communes] Préparation hermes impossible :', error)
    }
    if (!ready) {
      isRunning.value = false
      queue.value = []
      return
    }

    const failures: string[] = []
    const uncovered: string[] = []
    let done = 0
    try {
      while (queue.value.length > 0) {
        const item = queue.value.shift()
        if (!item || failed.has(item.traceId)) continue
        currentTraceId.value = item.traceId

        const trace = (await freshTraces()).find(t => t.id === item.traceId)
        if (!trace || trace.audit_status !== 'clean') continue
        if (!item.force && trace.communes_status !== 'none') continue

        try {
          const detection = await processOne(item.traceId)
          done += 1
          // La commande a écrit le statut côté backend : l'entrée en mémoire
          // doit le suivre sans relire le registre — la carte du circuit s'y
          // adosse, et le circuit passe au vert ou au jaune sans attendre la fin
          // du lot.
          useTracesStore().setCommunesStatus(item.traceId, detection.status)
          if (outsideCoverage.value[item.traceId]) uncovered.push(trace.name)
          console.log(
            `[communes] ${trace.name} : ${detection.passages.length} passage(s)` +
              (detection.status === 'partial' ? ' — détection partielle.' : '.'),
          )
        } catch (error) {
          // Les commandes hermes préfixent leurs erreurs par un code machine :
          // le message utilisateur est nettoyé, le code reste dans la console.
          const parsed = parseHermesError(error)
          failed.add(item.traceId)
          failures.push(`${trace.name} (${parsed.message})`)
          console.warn(
            `[communes] Détection impossible pour « ${trace.name} » [${parsed.code}] : ${parsed.message}`,
          )
        }
      }
    } finally {
      isRunning.value = false
      currentTraceId.value = null
      report(done, failures, uncovered)
    }
  }

  /**
   * Bilan de fin de tâche : **une seule** notification par catégorie, jamais une
   * par circuit — un rattrapage au lancement peut en enchaîner des dizaines.
   */
  function report(done: number, failures: string[], uncovered: string[]): void {
    const detected = done - uncovered.length
    if (detected > 0) {
      ui.showSuccess(
        detected === 1
          ? 'Communes détectées pour 1 circuit.'
          : `Communes détectées pour ${detected} circuits.`,
      )
    }
    if (uncovered.length > 0) {
      const first =
        uncovered.length > 1 ? `${uncovered[0]} +${uncovered.length - 1}` : uncovered[0]
      ui.showWarning(
        `Aucune commune détectée pour ${first} : la trace sort de la couverture du dataset servi.`,
      )
    }
    if (failures.length === 0) return
    const first = failures.length > 1 ? `${failures[0]} +${failures.length - 1}` : failures[0]
    ui.showWarning(`Détection des communes incomplète : ${done - uncovered.length} réussite(s), échec sur ${first}.`)
  }

  /**
   * Demande la détection pour un circuit.
   *
   * La trace passe en **tête** de file : la tâche en cours termine l'analyse
   * qu'elle a commencée, puis traite celle-ci — jamais deux analyses en
   * parallèle, ni deux écritures concurrentes. Sans tâche en cours, elle
   * démarre immédiatement.
   *
   * `force` sert au **recalcul** depuis la fenêtre de consultation : la trace
   * est déjà marquée détectée, mais l'opérateur veut actualiser le résultat.
   */
  function request(traceId: string, options: { force?: boolean } = {}): void {
    const force = options.force === true
    failed.delete(traceId)
    const pending = queue.value.find(item => item.traceId === traceId)
    if (pending) {
      if (force) pending.force = true
    } else if (currentTraceId.value !== traceId) {
      queue.value.unshift({ traceId, force })
    }
    void run('request')
  }

  /**
   * Rattrapage : met en file **toutes** les traces déjà nettoyées dont la
   * détection n'a pas abouti, puis attend la fin de la tâche.
   *
   * Rien à faire → aucune tâche ne démarre. Idempotent : une trace déjà en file
   * ou en cours d'analyse n'est pas réinscrite. Appelée au lancement **et** au
   * retour du service hermes (`App.vue`), le serveur dormant au démarrage.
   */
  async function startCatchUp(): Promise<void> {
    const pending = (await freshTraces()).filter(isEligible).map(t => t.id)
    if (pending.length === 0) return

    for (const traceId of pending) {
      if (traceId === currentTraceId.value) continue
      if (queue.value.some(item => item.traceId === traceId)) continue
      queue.value.push({ traceId, force: false })
    }
    await run('catch-up')
  }

  return {
    // État
    currentTraceId,
    isRunning,
    // Getters
    isPending,
    outsideCoverageDataset,
    // Actions
    loadPassages,
    request,
    resetDetection,
    startCatchUp,
  }
})
