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
 *
 * **`overpass.osm.ch` ne publie que l'extrait suisse.** Pour toute zone hors de
 * Suisse, il répond « aucun élément » en quelques dixièmes de seconde, sans
 * erreur ni `remark` — un faux vide indétectable côté réponse. C'est pourquoi
 * une réponse vide n'est acceptée qu'après avoir interrogé **tous** les miroirs
 * et seulement si aucun n'a échoué (voir `fetchCommunes`).
 */
const OVERPASS_ENDPOINTS = [
  'https://overpass-api.de/api/interpreter',
  'https://overpass.kumi.systems/api/interpreter',
  'https://overpass.private.coffee/api/interpreter',
  'https://overpass.osm.ch/api/interpreter',
]

/**
 * Délai par défaut accordé à chaque miroir, en secondes, quand le paramètre
 * `Communes.Overpass.delaiSec` est indisponible. Le client patiente
 * `DELAI_CLIENT_SUPPLEMENT_MS` de plus que le délai demandé au serveur, pour que
 * ce soit l'erreur du serveur (plus informative) qui clôt la tentative.
 */
const DEFAULT_DELAI_SEC = 60
const DELAI_CLIENT_SUPPLEMENT_MS = 5_000

/** Plancher de sécurité si le délai paramétré est inexploitable. */
const DELAI_MINIMUM_SEC = 10

/** Replis si un paramètre du module est absent ou inexploitable. */
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
  function buildQuery(
    bbox: [number, number, number, number],
    adminLevel: string,
    delaiSec: number,
  ): string {
    const [minLon, minLat, maxLon, maxLat] = bbox
    return (
      `[out:json][timeout:${delaiSec}];` +
      `relation["boundary"="administrative"]["admin_level"="${adminLevel}"]` +
      `(${minLat},${minLon},${maxLat},${maxLon});out geom;`
    )
  }

  /** Interroge un miroir et retourne ses `elements`. */
  async function fetchFrom(
    endpoint: string,
    query: string,
    timeoutMs: number,
  ): Promise<unknown[]> {
    const controller = new AbortController()
    const timer = window.setTimeout(() => controller.abort(), timeoutMs)
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
      const payload = (await response.json()) as { elements?: unknown[]; remark?: string }

      // Overpass signale ses erreurs d'exécution (délai dépassé, mémoire) par un
      // HTTP 200 portant un `remark` : c'est un **échec**, jamais un résultat
      // vide — le confondre avec « aucune commune » consignerait une détection
      // qui n'a pas eu lieu.
      if (typeof payload.remark === 'string' && payload.remark.trim()) {
        throw new Error(`Overpass : ${payload.remark}`)
      }
      if (!Array.isArray(payload.elements)) {
        throw new Error('Réponse Overpass inattendue (tableau `elements` absent).')
      }
      return payload.elements
    } finally {
      window.clearTimeout(timer)
    }
  }

  /**
   * Récupère les limites administratives autour d'une trace.
   *
   * Retourne les `elements` de la réponse Overpass, sérialisés en JSON : c'est
   * la charge transmise à la commande `communes_detect`. Le délai accordé à
   * chaque miroir vient du paramètre `Communes.Overpass.delaiSec`. Un tableau
   * **vide** n'est un résultat que s'il est **franc** — tous les miroirs ont
   * répondu, et aucun n'a échoué ; sinon la détection échoue et sera reprise,
   * plutôt que de consigner « aucune commune » sur une zone qu'on n'a pas su
   * interroger.
   */
  async function fetchCommunes(traceId: string): Promise<string> {
    const bbox = await queryBbox(traceId)
    const adminLevel = await settingString('Communes.Detection.adminLevel', DEFAULT_ADMIN_LEVEL)
    const delaiSec = Math.max(
      DELAI_MINIMUM_SEC,
      Math.round(await settingNumber('Communes.Overpass.delaiSec', DEFAULT_DELAI_SEC)),
    )
    const query = buildQuery(bbox, adminLevel, delaiSec)
    const timeoutMs = delaiSec * 1000 + DELAI_CLIENT_SUPPLEMENT_MS

    isLoading.value = true
    const failures: string[] = []
    /** Première réponse valide mais vide, gardée en réserve. */
    let emptyPayload: string | null = null
    try {
      for (const endpoint of OVERPASS_ENDPOINTS) {
        try {
          const elements = await fetchFrom(endpoint, query, timeoutMs)
          // Une réponse non vide est la réponse utile : elle prime sur toute
          // réponse vide, même obtenue plus tôt.
          if (elements.length > 0) {
            return JSON.stringify(elements)
          }
          // Vide : la zone est sans commune… ou le miroir ne la couvre pas.
          // On garde de côté, et on interroge les suivants.
          emptyPayload ??= JSON.stringify(elements)
        } catch (error) {
          const reason = error instanceof Error ? error.message : String(error)
          failures.push(`${new URL(endpoint).host} (${reason})`)
          console.warn(`[communes] Miroir Overpass indisponible : ${endpoint} — ${reason}`)
        }
      }

      // Tous les miroirs ont répondu sans erreur : la zone ne contient
      // réellement aucune commune, le vide est un résultat.
      if (emptyPayload !== null && failures.length === 0) {
        return emptyPayload
      }

      // Sinon le vide est ambigu — un miroir partiel (osm.ch) ou une panne a pu
      // le produire. Mieux vaut échouer : la détection sera relancée.
      throw new Error(
        emptyPayload !== null
          ? `Réponse vide ambiguë (${failures.length} miroir(s) en échec) : ${failures.join(', ')}`
          : `Aucun miroir Overpass n'a répondu : ${failures.join(', ')}`,
      )
    } finally {
      isLoading.value = false
    }
  }

  return { isLoading, fetchCommunes }
}
