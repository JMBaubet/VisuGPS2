/**
 * Helpers purs du client `hermes` (aucune dépendance UI).
 *
 * - décodage des erreurs préfixées par un code machine (`HERMES_OFFLINE: …`) ;
 * - description de l'état affiché par le chip de la toolbar ;
 * - validation et mesure des polygones custom (dessin Mapbox) ;
 * - limites du serveur pour les avertissements souples de l'IHM.
 *
 * Les fonctions sont sans état et sans accès réseau : les stores et les
 * composants les consomment, ce qui les rend testables et réutilisables.
 */

import { toRadians } from './geo'

/** Code machine d'une erreur hermes, ou `HERMES_UNKNOWN`. */
export interface HermesErrorInfo {
  code: string
  message: string
}

/**
 * Décode une erreur remontée par une commande Tauri `hermes_*`.
 *
 * Les commandes renvoient `Result<T, String>` et préfixent la chaîne par un
 * code (`HERMES_OFFLINE: Connexion…`). Une erreur sans code connu est classée
 * `HERMES_UNKNOWN` : l'IHM affiche le message tel quel.
 */
export function parseHermesError(error: unknown): HermesErrorInfo {
  const raw = error instanceof Error ? error.message : String(error)
  const trimmed = raw.trim()
  const match = /^(HERMES_[A-Z_]+):\s*(.+)$/s.exec(trimmed)
  if (match) {
    return { code: match[1], message: match[2] }
  }
  return { code: 'HERMES_UNKNOWN', message: trimmed || 'Erreur hermes inconnue.' }
}

/** État de connexion au serveur, tel que porté par le store. */
export type HermesServerState = 'unknown' | 'online' | 'offline' | 'waking'

/** Mode de l'outil de dessin de polygones sur la carte Hermes. */
export type HermesDrawMode = 'idle' | 'create' | 'edit'

/** Vue minimale d'un dataset nécessaire à l'affichage du chip. */
export interface HermesDatasetStatusLike {
  id: string
  status: string
  stale: boolean
}

/** Affichage prêt à rendre pour le chip de la toolbar. */
export interface HermesStatusDisplay {
  icon: string
  color: string
  label: string
  tooltip: string[]
}

function stateLabel(state: HermesServerState): string {
  switch (state) {
    case 'online':
      return 'en ligne'
    case 'offline':
      return 'hors ligne'
    case 'waking':
      return 'réveil en cours'
    default:
      return 'état inconnu'
  }
}

/**
 * Décrit le chip « état serveur + dataset actif » de la toolbar de l'Accueil.
 *
 * Le libellé est l'identifiant technique du dataset — le serveur n'expose pas
 * de nom d'affichage dans `GET /datasets`. La couleur résume l'état : vert
 * (serveur en ligne et dataset prêt), orange (aucun dataset, préparation en
 * cours ou attente), rouge (serveur hors ligne ou build échoué), gris (état
 * inconnu).
 */
export function describeHermesStatus(
  state: HermesServerState,
  dataset: HermesDatasetStatusLike | null,
  lastError: string | null,
  baseUrl: string,
): HermesStatusDisplay {
  const tooltip = [
    `Serveur hermes : ${stateLabel(state)}.`,
    `URL : ${baseUrl}`,
  ]

  if (dataset) {
    const stale = dataset.stale ? ' — obsolète' : ''
    tooltip.push(`Dataset actif : ${dataset.id} (${dataset.status}${stale}).`)
  } else {
    tooltip.push('Aucun dataset actif — ouvrez Hermes pour en choisir un.')
  }
  if (lastError) {
    tooltip.push(`Dernière erreur : ${lastError}`)
  }
  tooltip.push('Cliquer pour ouvrir la vue Hermes.')

  if (state === 'waking') {
    return {
      icon: 'mdi-progress-clock',
      color: 'orange',
      label: 'Réveil…',
      tooltip,
    }
  }
  if (state === 'offline') {
    return {
      icon: 'mdi-server-off',
      color: 'red',
      label: 'Hors ligne',
      tooltip,
    }
  }
  if (state === 'unknown') {
    return {
      icon: 'mdi-server',
      color: 'grey',
      label: 'Hermes',
      tooltip,
    }
  }
  if (!dataset) {
    return {
      icon: 'mdi-server-network',
      color: 'orange',
      label: 'Aucun dataset',
      tooltip,
    }
  }
  if (dataset.status === 'ready') {
    return {
      icon: 'mdi-server-network',
      color: 'green',
      label: dataset.id,
      tooltip,
    }
  }
  if (dataset.status === 'preparing') {
    return {
      icon: 'mdi-progress-clock',
      color: 'orange',
      label: `${dataset.id} · préparation`,
      tooltip,
    }
  }
  return {
    icon: 'mdi-server-alert',
    color: 'red',
    label: `${dataset.id} · échec`,
    tooltip,
  }
}

/**
 * Limites par défaut du serveur, pour les **avertissements souples** de l'IHM.
 * Le serveur reste l'autorité : ses messages 400 sont affichés tels quels.
 */
export const DATASET_LIMITS = {
  maxRegions: 4,
  maxZones: 100,
  maxAreaKm2: 50_000,
} as const

/**
 * Génère un identifiant de polygone à partir d'un nom : minuscules, sans
 * accents, séparateurs `-`, préfixe `ma-zone-`. La longueur totale reste sous
 * la limite serveur de 64 caractères.
 */
export function slugifyPolygonName(name: string): string {
  const slug = name
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .replace(/^ma-zone-/, '')
    .slice(0, 52)
  return slug ? `ma-zone-${slug}` : ''
}

/** Identifiant de polygone valide : slug ASCII, `index` réservé. */
export function isValidPolygonSlug(id: string): boolean {
  return /^[A-Za-z0-9_-]{1,64}$/.test(id) && id !== 'index'
}

/** Aire sphérique d'un anneau, en km² (formule de Turf `ringArea`). */
function ringAreaKm2(ring: number[][]): number {
  if (ring.length < 3) return 0
  const earthRadiusKm = 6371.0088
  let total = 0
  for (let index = 0; index < ring.length; index += 1) {
    const [lon1, lat1] = ring[index]
    const [lon2, lat2] = ring[(index + 1) % ring.length]
    total +=
      toRadians(lon2 - lon1) *
      (2 + Math.sin(toRadians(lat1)) + Math.sin(toRadians(lat2)))
  }
  return (total * earthRadiusKm * earthRadiusKm) / 2
}

/**
 * Aire d'un polygone GeoJSON en km². Les trous éventuels sont soustraits ; le
 * serveur les refuse, mais la mesure reste juste pendant l'édition.
 */
export function polygonAreaKm2(geometry: GeoJSON.Polygon): number {
  const rings = geometry.coordinates ?? []
  if (rings.length === 0) return 0
  const outer = Math.abs(ringAreaKm2(rings[0]))
  const holes = rings
    .slice(1)
    .reduce((sum, ring) => sum + Math.abs(ringAreaKm2(ring)), 0)
  return Math.max(0, outer - holes)
}

/** Résultat de la validation locale d'une géométrie de polygone. */
export interface PolygonValidation {
  ok: boolean
  message: string | null
}

/**
 * Valide une géométrie avant envoi : `Polygon` uniquement, sans trous, anneau
 * fermé, 4 à 1000 sommets, coordonnées WGS84 finies.
 *
 * Le serveur arrondit à 5 décimales et ferme silencieusement un anneau ouvert ;
 * l'IHM est plus stricte pour signaler tout de suite une géométrie inattendue.
 */
export function validatePolygonGeometry(
  geometry: GeoJSON.Geometry | null | undefined,
): PolygonValidation {
  if (!geometry) {
    return { ok: false, message: 'Aucune géométrie à enregistrer.' }
  }
  if (geometry.type !== 'Polygon') {
    return {
      ok: false,
      message: 'La géométrie doit être un Polygon (MultiPolygon non accepté).',
    }
  }
  const rings = geometry.coordinates
  if (!Array.isArray(rings) || rings.length === 0) {
    return { ok: false, message: 'Le polygone est vide.' }
  }
  if (rings.length > 1) {
    return { ok: false, message: 'Les trous ne sont pas acceptés par le serveur.' }
  }
  const ring = rings[0]
  if (!Array.isArray(ring) || ring.length < 4) {
    return {
      ok: false,
      message: 'Un polygone doit avoir au moins 4 sommets (anneau fermé).',
    }
  }
  if (ring.length > 1000) {
    return { ok: false, message: 'Un polygone est limité à 1000 sommets.' }
  }
  const first = ring[0]
  const last = ring[ring.length - 1]
  if (!first || !last) {
    return { ok: false, message: 'Anneau de polygone invalide.' }
  }
  if (first[0] !== last[0] || first[1] !== last[1]) {
    return {
      ok: false,
      message: "L'anneau doit être fermé (premier et dernier point identiques).",
    }
  }
  for (const point of ring) {
    const [lon, lat] = point
    if (
      !Number.isFinite(lon) ||
      !Number.isFinite(lat) ||
      lon < -180 ||
      lon > 180 ||
      lat < -90 ||
      lat > 90
    ) {
      return {
        ok: false,
        message: 'Coordonnées invalides : WGS84 (lon ±180, lat ±90) attendu.',
      }
    }
  }
  return { ok: true, message: null }
}

/** Formate une bbox `[lon_min, lat_min, lon_max, lat_max]` pour l'affichage. */
export function formatBbox(
  bbox: [number, number, number, number],
): string {
  const [minLon, minLat, maxLon, maxLat] = bbox
  return `${minLon.toFixed(2)}, ${minLat.toFixed(2)} → ${maxLon.toFixed(2)}, ${maxLat.toFixed(2)}`
}

/** Union de plusieurs bbox, ou `null` si la liste est vide. */
export function unionBboxes(
  bboxes: [number, number, number, number][],
): [number, number, number, number] | null {
  if (bboxes.length === 0) return null
  let minLon = Number.POSITIVE_INFINITY
  let minLat = Number.POSITIVE_INFINITY
  let maxLon = Number.NEGATIVE_INFINITY
  let maxLat = Number.NEGATIVE_INFINITY
  for (const bbox of bboxes) {
    minLon = Math.min(minLon, bbox[0])
    minLat = Math.min(minLat, bbox[1])
    maxLon = Math.max(maxLon, bbox[2])
    maxLat = Math.max(maxLat, bbox[3])
  }
  if (!Number.isFinite(minLon) || !Number.isFinite(minLat)) return null
  return [minLon, minLat, maxLon, maxLat]
}

/** Formate une durée en secondes pour les progressions (build, réveil). */
export function formatSeconds(value: number): string {
  if (!Number.isFinite(value) || value < 0) return '—'
  if (value < 60) return `${Math.round(value)} s`
  const minutes = Math.floor(value / 60)
  const seconds = Math.round(value % 60)
  return `${minutes} min ${String(seconds).padStart(2, '0')} s`
}
