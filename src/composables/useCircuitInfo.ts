/**
 * Chargement des informations de synthèse d'un circuit (volet Info).
 *
 * Une seule commande Tauri agrège les quatre artefacts de la trace — audit,
 * passages multiples, altimétrie et communes — sans transférer les archives
 * complètes. Le résultat est mis en cache pour l'instance de carte : rouvrir
 * le volet Info ne relance pas la lecture disque, et un retour sur l'accueil
 * remonte les cartes, donc rafraîchit l'état.
 */

import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

/** Point allégé du profil d'altitude (distance en m, altitude en m). */
export interface CircuitProfilePoint {
  d: number
  ele: number
}

/** Montée résumée pour la sparkline du volet Info. */
export interface CircuitClimbInfo {
  index: number
  startKm: number
  endKm: number
  avgSlopePct: number
  isMerged: boolean
  isSeparated: boolean
}

/** Compteurs des aller-retours et des rond-points, avec la date de l'audit. */
export interface CircuitAuditInfo {
  arTotal: number
  arTreated: number
  arPending: number
  rpTotal: number
  rpTreated: number
  rpPending: number
  updatedAt: string
}

/** Compteurs des segments de passages multiples. */
export interface CircuitMultirideInfo {
  segmentsTotal: number
  segmentsTreated: number
  segmentsPending: number
  updatedAt: string
}

/** Analyse altimétrique : montées, profil et bornes. */
export interface CircuitAltimetrieInfo {
  simpleClimbs: number
  mergedClimbs: number
  separatedClimbs: number
  /** L'opérateur a validé l'analyse (sauvegarde depuis la vue Altimétrie). */
  validated: boolean
  updatedAt: string
  profile: CircuitProfilePoint[]
  climbs: CircuitClimbInfo[]
  altMinM: number
  altMaxM: number
  totalDistanceM: number
}

/** Nombre de communes distinctes traversées. */
export interface CircuitCommunesInfo {
  communeCount: number
}

/** Réponse de la commande `get_trace_info`. */
export interface CircuitInfo {
  audit: CircuitAuditInfo | null
  multiride: CircuitMultirideInfo | null
  altimetrie: CircuitAltimetrieInfo | null
  communes: CircuitCommunesInfo | null
}

export function useCircuitInfo() {
  const summary = ref<CircuitInfo | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)
  let loadedTraceId: string | null = null
  let request: Promise<void> | null = null

  /**
   * Charge les informations d'une trace, une seule fois par instance de carte.
   * Un échec est exposé dans `error` ; un nouveau clic sur Info réessaie.
   */
  async function load(traceId: string): Promise<void> {
    if (loadedTraceId === traceId && summary.value) return
    if (request) return request

    loading.value = true
    error.value = null
    request = (async () => {
      try {
        summary.value = await invoke<CircuitInfo>('get_trace_info', { traceId })
        loadedTraceId = traceId
      } catch (cause) {
        error.value =
          typeof cause === 'string'
            ? cause
            : 'Informations du circuit indisponibles.'
        summary.value = null
      } finally {
        loading.value = false
        request = null
      }
    })()

    return request
  }

  return {
    summary,
    loading,
    error,
    load,
  }
}
