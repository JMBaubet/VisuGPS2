/**
 * Contrat réseau de la détection des communes : interroge l'API Overpass
 * d'OpenStreetMap pour récupérer les limites administratives autour d'une trace.
 *
 * Le réseau reste côté front (WebView), comme pour OpenRouteService
 * (`useAuditOrs`) : la réponse brute est ensuite transmise à la commande
 * `communes_detect`, qui se charge de la géométrie, de l'écriture du fichier et
 * de l'état. Seule la **boîte englobante de la trace** est envoyée au serveur —
 * jamais la trace elle-même.
 */

import { ref } from 'vue'
import { useSettingsStore } from '../stores/settings'
import { useTracesStore } from '../stores/traces'
import { bboxOfCoordinates } from '../utils/geo'

/**
 * Miroirs Overpass, essayés en cascade : un miroir en panne (erreur HTTP, délai
 * dépassé, réseau) laisse la place au suivant. Le serveur principal est
 * régulièrement saturé.
 */
const OVERPASS_ENDPOINTS = [
  'https://overpass-api.de/api/interpreter',
  'https://overpass.kumi.systems/api/interpreter',
  'https://overpass.private.coffee/api/interpreter',
  'https://overpass.osm.ch/api/interpreter',
]

/**
 * Délai accordé au serveur par la requête Overpass (s), et délai d'abandon du
 * client (ms) — volontairement plus long, pour que ce soit l'erreur du serveur
 * (504, plus informative) qui clôt la tentative.
 */
const OVERPASS_QUERY_TIMEOUT_S = 25
const OVERPASS_FETCH_TIMEOUT_MS = 30_000

/** Replis si un paramètre `Communes.Detection.*` est absent ou inexploitable. */
const DEFAULT_BBOX_BUFFER_DEG = 0.01
const DEFAULT_ADMIN_LEVEL = '8'

export function useCommunesOverpass() {
  const settingsStore = useSettingsStore()
  const tracesStore = useTracesStore()

  /** Vrai pendant une requête (tous miroirs confondus). */
  const isLoading = ref(false)

  /** Valeur numérique d'un paramètre (repli si absente ou non numérique). */
  async function settingNumber(path: string, fallback: number): Promise<number> {
    try {
      const value = Number(await settingsStore.getSettingValue(path))
      return Number.isFinite(value) ? value : fallback
    } catch {
      return fallback
    }
  }

  /** Valeur texte d'un paramètre (repli si absente ou vide). */
  async function settingString(path: string, fallback: string): Promise<string> {
    try {
      const value = await settingsStore.getSettingValue(path)
      return typeof value === 'string' && value.trim() ? value : fallback
    } catch {
      return fallback
    }
  }

  /**
   * Zone interrogée : boîte englobante de la trace, élargie de
   * `Communes.Detection.bboxBufferDeg` — une commune traversée en bordure de
   * trace peut avoir son polygone entièrement au-delà de la boîte brute.
   */
  async function queryBbox(traceId: string): Promise<[number, number, number, number]> {
    const feature = await tracesStore.getTraceGeometry(traceId)
    const coordinates = (feature?.geometry as { coordinates?: number[][] } | undefined)
      ?.coordinates
    const bbox = coordinates ? bboxOfCoordinates(coordinates) : null
    if (!bbox) {
      throw new Error('Géométrie de trace indisponible.')
    }

    const buffer = await settingNumber(
      'Communes.Detection.bboxBufferDeg',
      DEFAULT_BBOX_BUFFER_DEG,
    )
    return [bbox[0] - buffer, bbox[1] - buffer, bbox[2] + buffer, bbox[3] + buffer]
  }

  /**
   * Requête Overpass QL : les relations de limite administrative du niveau visé,
   * dans la zone. Overpass attend `(S, W, N, E)` là où la bbox GeoJSON est
   * `[minLon, minLat, maxLon, maxLat]`.
   */
  function buildQuery(bbox: [number, number, number, number], adminLevel: string): string {
    const [minLon, minLat, maxLon, maxLat] = bbox
    return (
      `[out:json][timeout:${OVERPASS_QUERY_TIMEOUT_S}];` +
      `relation["boundary"="administrative"]["admin_level"="${adminLevel}"]` +
      `(${minLat},${minLon},${maxLat},${maxLon});out geom;`
    )
  }

  /** Interroge un miroir et retourne ses `elements`. */
  async function fetchFrom(endpoint: string, query: string): Promise<unknown[]> {
    const controller = new AbortController()
    const timer = window.setTimeout(() => controller.abort(), OVERPASS_FETCH_TIMEOUT_MS)
    try {
      const response = await fetch(endpoint, {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({ data: query }),
        signal: controller.signal,
      })
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`)
      }
      const payload = (await response.json()) as { elements?: unknown[] }
      return Array.isArray(payload.elements) ? payload.elements : []
    } finally {
      window.clearTimeout(timer)
    }
  }

  /**
   * Récupère les limites administratives autour d'une trace.
   *
   * Retourne les `elements` de la réponse Overpass, sérialisés en JSON : c'est
   * la charge transmise à la commande `communes_detect`. Un tableau **vide** est
   * une réponse valide — aucune commune connue dans la zone.
   */
  async function fetchCommunes(traceId: string): Promise<string> {
    const bbox = await queryBbox(traceId)
    const adminLevel = await settingString('Communes.Detection.adminLevel', DEFAULT_ADMIN_LEVEL)
    const query = buildQuery(bbox, adminLevel)

    isLoading.value = true
    const failures: string[] = []
    try {
      for (const endpoint of OVERPASS_ENDPOINTS) {
        try {
          return JSON.stringify(await fetchFrom(endpoint, query))
        } catch (error) {
          const reason = error instanceof Error ? error.message : String(error)
          failures.push(`${new URL(endpoint).host} (${reason})`)
          console.warn(`[communes] Miroir Overpass indisponible : ${endpoint} — ${reason}`)
        }
      }
      throw new Error(`Aucun miroir Overpass n'a répondu : ${failures.join(', ')}`)
    } finally {
      isLoading.value = false
    }
  }

  return { isLoading, fetchCommunes }
}
