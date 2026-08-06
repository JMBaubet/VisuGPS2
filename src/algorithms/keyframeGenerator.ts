/**
 * Génération et interpolation des keyframes de la caméra d'édition.
 *
 * Module isolé (spec §3.2) : ne dépend ni de l'UI (Vue/Vuetify) ni de
 * Mapbox, uniquement de `utils/geo`. Il reçoit une Feature LineString et
 * produit un jeu de keyframes. Si l'algorithme de génération ne donne pas
 * satisfaction, il peut être repensé sans impact sur le reste de
 * l'application.
 *
 * Format de sortie conforme au contrat JSON figé (spec §3.4). Aucune
 * persistance pour l'instant : le jeu est régénéré à chaque entrée dans la
 * vue d'édition.
 *
 * Deux familles de fonctions :
 *   1. `generateKeyframes` — produit le jeu (échantillonnage régulier le
 *      long de la trace ; chaque keyframe place la caméra sur la trace).
 *   2. Helpers purs d'interpolation (`findSegment`, `interpolateCam`,
 *      `interpolateTraceur`) — consommés par la boucle de lecture.
 */

import { bearing, haversineMeters } from '../utils/geo'

// --- Types (miroir du format JSON figé, spec §3.4) ---

/** État de la caméra à un instant donné (paramètres Mapbox). */
export interface CamState {
  lng: number
  lat: number
  zoom: number
  bearing: number
  pitch: number
}

/** Position du marqueur (traceur) à un instant donné. */
export interface TraceurPoint {
  lng: number
  lat: number
  altitude: number | null
}

/** Un point de rendez-vous (keyframe) sur la trajectoire. */
export interface Keyframe {
  /** Temps absolu depuis le départ (ms). */
  time_ms: number
  /** Distance cumulée sur la trace depuis le départ (m). */
  distance_from_start_m: number
  /** État de la caméra à ce keyframe. */
  cam: CamState
  /** Position du traceur à ce keyframe. */
  traceur: TraceurPoint
}

/** Jeu complet de keyframes pour une trace. */
export interface KeyframeSet {
  trace_id: string
  total_distance_m: number
  total_duration_ms: number
  viewport: { width: number; height: number }
  sample_rate_m: number
  keyframes: Keyframe[]
}

// --- Paramètres ---

/** Pas d'échantillonnage des keyframes le long de la trace (m). */
export const KEYFRAME_STEP_M = 1000

/** Vitesse par défaut : 4000 ms/km, soit 4 ms par mètre (spec §7). */
export const MS_PER_METER = 4

/** Paramètres de caméra par défaut (spec §7). */
export const DEFAULT_CAM_ZOOM = 16
export const DEFAULT_CAM_PITCH = 60

/** Viewport de référence pour l'export vidéo (spec §3.4). */
const REFERENCE_VIEWPORT = { width: 1920, height: 1080 }

// --- Génération ---

/**
 * Génère un jeu de keyframes par échantillonnage régulier le long d'une
 * trace LineString.
 *
 * Principe :
 *   - calcul de la distance cumulée point à point (Haversine) ;
 *   - échantillonnage d'un keyframe tous les `sampleStepM` (interpolation
 *     exacte de la position sur la polyligne à la distance cible) ;
 *   - la caméra est placée sur la trace (cam = traceur), avec le cap
 *     orienté vers le keyframe suivant (le dernier reprend le cap précédent).
 *
 * Algorithme volontairement simple (MVP) : il garantit que la caméra suit
 * fidèlement la trace pendant la lecture, le marqueur restant au centre
 * aux keyframes. Il sera remplacé à terme par l'algorithme de frustum
 * (spec §3.3) sans impacter le reste de l'application.
 *
 * @param traceId     - Identifiant de la trace (reporté dans le jeu).
 * @param feature     - Feature GeoJSON LineString `[lon, lat]` de la trace.
 * @param sampleStepM - Pas d'échantillonnage en mètres (défaut 250 m).
 * @returns Le jeu de keyframes, ou `null` si la trace est vide.
 */
export function generateKeyframes(
  traceId: string,
  feature: GeoJSON.Feature,
  sampleStepM: number = KEYFRAME_STEP_M,
): KeyframeSet | null {
  const coords = extractLineCoordinates(feature)
  if (coords.length === 0) return null

  // 1. Construire la polyligne indexée par distance cumulée.
  const poly = buildPolyline(coords)
  if (poly.length === 0) return null

  const totalDistance = poly[poly.length - 1].d

  // 2. Échantillonner un keyframe tous les `sampleStepM`, plus le point final.
  const sampleDistances: number[] = []
  for (let d = 0; d <= totalDistance; d += sampleStepM) {
    sampleDistances.push(d)
  }
  // Garantir l'arrivée (le dernier échantillon peut dépasser un peu).
  if (sampleDistances[sampleDistances.length - 1] !== totalDistance) {
    sampleDistances.push(totalDistance)
  }

  // 3. Construire les keyframes (position + temps). Le bearing est calculé
  //    dans une seconde passe car il dépend du keyframe suivant.
  const rawKf: { p: { lng: number; lat: number }; d: number; t: number }[] =
    sampleDistances.map(d => ({
      p: pointAtDistance(poly, d),
      d,
      t: d * MS_PER_METER,
    }))

  const keyframes: Keyframe[] = rawKf.map((k, i) => {
    const next = rawKf[i + 1] ?? rawKf[i] // dernier keyframe : cap vers lui-même (corrigé ci-après)
    const b = bearing(k.p.lat, k.p.lng, next.p.lat, next.p.lng)
    return {
      time_ms: k.t,
      distance_from_start_m: k.d,
      cam: {
        lng: k.p.lng,
        lat: k.p.lat,
        zoom: DEFAULT_CAM_ZOOM,
        bearing: b,
        pitch: DEFAULT_CAM_PITCH,
      },
      traceur: { lng: k.p.lng, lat: k.p.lat, altitude: null },
    }
  })

  // Dernier keyframe : reprend le cap du précédent (pas de « suivant »).
  if (keyframes.length >= 2) {
    keyframes[keyframes.length - 1].cam.bearing =
      keyframes[keyframes.length - 2].cam.bearing
  }

  return {
    trace_id: traceId,
    total_distance_m: totalDistance,
    total_duration_ms: totalDistance * MS_PER_METER,
    viewport: { ...REFERENCE_VIEWPORT },
    sample_rate_m: sampleStepM,
    keyframes,
  }
}

// --- Interpolation (consommée par la boucle de lecture) ---

/**
 * Recherche les keyframes encadrant un instant donné (recherche
 * dichotomique, O(log n)).
 *
 * @returns `{ prev, next, ratio }` où `ratio` ∈ [0, 1] est la position
 *          normalisée entre `prev` et `next`. Avant le premier keyframe,
 *          renvoie `prev = next = keyframes[0]` et `ratio = 0`. Après le
 *          dernier, `prev = next = dernier` et `ratio = 0`.
 */
export function findSegment(
  keyframes: Keyframe[],
  timeMs: number,
): { prev: Keyframe; next: Keyframe; ratio: number } {
  const n = keyframes.length
  if (n === 0) throw new Error('findSegment: keyframes vides')
  if (timeMs <= keyframes[0].time_ms) {
    return { prev: keyframes[0], next: keyframes[0], ratio: 0 }
  }
  const last = keyframes[n - 1]
  if (timeMs >= last.time_ms) {
    return { prev: last, next: last, ratio: 0 }
  }

  let lo = 0
  let hi = n - 1
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1
    if (keyframes[mid].time_ms <= timeMs) lo = mid
    else hi = mid
  }
  const prev = keyframes[lo]
  const next = keyframes[hi]
  const span = next.time_ms - prev.time_ms
  const ratio = span > 0 ? (timeMs - prev.time_ms) / span : 0
  return { prev, next, ratio }
}

/**
 * Interpole l'état de la caméra entre deux keyframes.
 *
 * Le bearing est interpolé sur le cercle (gestion du wrap 360°) pour
 * éviter une rotation complète quand le cap passe de 359° à 1°.
 *
 * @param segment - Segment renvoyé par `findSegment`.
 */
export function interpolateCam(segment: {
  prev: Keyframe
  next: Keyframe
  ratio: number
}): CamState {
  const { prev, next, ratio } = segment
  const lerp = (a: number, b: number) => a + (b - a) * ratio
  return {
    lng: lerp(prev.cam.lng, next.cam.lng),
    lat: lerp(prev.cam.lat, next.cam.lat),
    zoom: lerp(prev.cam.zoom, next.cam.zoom),
    pitch: lerp(prev.cam.pitch, next.cam.pitch),
    bearing: lerpAngle(prev.cam.bearing, next.cam.bearing, ratio),
  }
}

/**
 * Interpole la position du traceur entre deux keyframes.
 *
 * L'altitude est renvoyée à `null` (données non exposées côté frontend
 * dans cette itération).
 */
export function interpolateTraceur(segment: {
  prev: Keyframe
  next: Keyframe
  ratio: number
}): TraceurPoint {
  const { prev, next, ratio } = segment
  const lerp = (a: number, b: number) => a + (b - a) * ratio
  return {
    lng: lerp(prev.traceur.lng, next.traceur.lng),
    lat: lerp(prev.traceur.lat, next.traceur.lat),
    altitude: null,
  }
}

// --- Polyligne indexée par distance (réutilisée pour le marker) ---

/** Sommet de polyligne indexé par sa distance cumulée depuis le départ (m). */
export interface PolyVertex {
  lng: number
  lat: number
  /** Distance cumulée depuis le départ de la trace (m). */
  d: number
  /** Altitude du point (m), `null` si non disponible. */
  altitude: number | null
}

/**
 * Construit la polyligne indexée par distance cumulée à partir d'une
 * Feature LineString. Les sommets consécutifs identiques (distance nulle)
 * sont filtrés pour éviter des divisions par zéro lors de l'échantillonnage.
 *
 * Exposée publiquement : le marker de la vue d'édition l'utilise pour
 * avancer **le long de la trace réelle** (tous les points GPX) plutôt
 * qu'entre les keyframes échantillonnés (sinon il tracerait des cordes
 * droites à travers les virages).
 */
export function buildTracePolyline(feature: GeoJSON.Feature): PolyVertex[] {
  return buildPolyline(extractLineCoordinates(feature))
}

/**
 * Construit la polyligne indexée par distance cumulée à partir des points
 * riches retournés par le backend (`get_trace_points`).
 *
 * Contrairement à `buildTracePolyline`, cette fonction porte l'**altitude**
 * réelle de chaque point. Les distances cumulées sont recalculées en 2D
 * (Haversine) pour rester cohérentes avec les keyframes (qui utilisent des
 * distances 2D). L'altitude n'est pas utilisée pour le calcul de distance.
 *
 * @param points - Tableau de `TracePoint` (lat, lon, alt, distance_m).
 */
export function buildTracePolylineFromPoints(
  points: { lat: number; lon: number; alt: number | null; distance_m: number }[],
): PolyVertex[] {
  if (points.length === 0) return []
  const poly: PolyVertex[] = []
  let acc = 0
  for (let i = 0; i < points.length; i++) {
    const p = points[i]
    if (i === 0) {
      poly.push({ lng: p.lon, lat: p.lat, d: 0, altitude: p.alt })
      continue
    }
    const prev = points[i - 1]
    const seg = haversineMeters(prev.lat, prev.lon, p.lat, p.lon)
    if (seg <= 0) continue // filtre les doublons
    acc += seg
    poly.push({ lng: p.lon, lat: p.lat, d: acc, altitude: p.alt })
  }
  return poly
}

/**
 * Échantillonne la position exacte sur la polyligne à une distance cumulée
 * donnée (interpolation linéaire dans le segment contenant `distanceM`).
 * Clamp aux extrémités. L'altitude est interpolée linéairement entre les
 * deux sommets encadrants.
 */
export function samplePolylineAt(
  poly: PolyVertex[],
  distanceM: number,
): { lng: number; lat: number; altitude: number | null } | null {
  if (poly.length === 0) return null
  if (poly.length === 1)
    return { lng: poly[0].lng, lat: poly[0].lat, altitude: poly[0].altitude }
  const total = poly[poly.length - 1].d
  const d = Math.min(Math.max(distanceM, 0), total)
  return pointAtDistance(poly, d)
}

// --- Internes : primitives ---

/** Extrait les coordonnées `[lon, lat]` d'une Feature LineString. */
function extractLineCoordinates(feature: GeoJSON.Feature): [number, number][] {
  const geom = feature.geometry
  if (geom && geom.type === 'LineString') {
    return geom.coordinates as [number, number][]
  }
  return []
}

/**
 * Construit la polyligne indexée par distance cumulée.
 *
 * Les sommets consécutifs identiques (distance nulle) sont filtrés pour
 * éviter des divisions par zéro lors de l'échantillonnage.
 */
function buildPolyline(coords: [number, number][]): PolyVertex[] {
  const poly: PolyVertex[] = []
  let acc = 0
  for (let i = 0; i < coords.length; i++) {
    const [lng, lat] = coords[i]
    if (i === 0) {
      poly.push({ lng, lat, d: 0, altitude: null })
      continue
    }
    const [prevLng, prevLat] = coords[i - 1]
    const seg = haversineMeters(prevLat, prevLng, lat, lng)
    if (seg <= 0) continue // filtre les doublons
    acc += seg
    poly.push({ lng, lat, d: acc, altitude: null })
  }
  return poly
}

/**
 * Calcule la position exacte sur la polyligne à une distance cumulée donnée
 * (interpolation linéaire dans le segment contenant `d`).
 * L'altitude est interpolée linéairement entre les deux sommets.
 */
function pointAtDistance(
  poly: PolyVertex[],
  d: number,
): { lng: number; lat: number; altitude: number | null } {
  if (d <= 0) return { lng: poly[0].lng, lat: poly[0].lat, altitude: poly[0].altitude }
  const last = poly[poly.length - 1]
  if (d >= last.d) return { lng: last.lng, lat: last.lat, altitude: last.altitude }

  // Recherche du segment contenant d.
  let lo = 0
  let hi = poly.length - 1
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1
    if (poly[mid].d <= d) lo = mid
    else hi = mid
  }
  const a = poly[lo]
  const b = poly[hi]
  const span = b.d - a.d
  const ratio = span > 0 ? (d - a.d) / span : 0

  // Interpolation de l'altitude (null si l'un des sommets n'a pas d'altitude).
  let altitude: number | null = null
  if (a.altitude !== null && b.altitude !== null) {
    altitude = a.altitude + (b.altitude - a.altitude) * ratio
  }

  return {
    lng: a.lng + (b.lng - a.lng) * ratio,
    lat: a.lat + (b.lat - a.lat) * ratio,
    altitude,
  }
}

/** Interpolation linéaire d'angles (degrés) avec gestion du wrap 360°. */
function lerpAngle(a: number, b: number, t: number): number {
  const diff = ((b - a + 540) % 360) - 180 // écart le plus court sur [-180, 180]
  return (a + diff * t + 360) % 360
}
