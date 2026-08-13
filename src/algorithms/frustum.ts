/**
 * Algorithme de frustum (spec §3.3) — génération des keyframes par placement
 * récursif basé sur la visibilité.
 *
 * Module isolé (spec §3.2) : aucune dépendance UI ni Mapbox, uniquement
 * `utils/geo` et les primitives de `keyframeGenerator.ts`. Produit un
 * `KeyframeSet` au même format JSON figé (spec §3.4) que `generateKeyframes`.
 *
 * Principe :
 *   - Départ : keyframes A (départ) et Z (arrivée), caméra sur la trace,
 *     zoom/pitch par défaut, bearing = cap direct A→Z.
 *   - Pour chaque segment [A, Z], on parcourt les points GPX intermédiaires :
 *     la caméra vole le long de la **corde A→Z** ; le traceur suit la trace
 *     courbe. On vérifie que chaque point reste dans le **champ de vision**
 *     (projection frustum) et n'est pas **masqué par le relief** (ligne de
 *     visée contre les points intermédiaires de la trace).
 *   - Si un point n'est pas visible :
 *       • latéralement (virage/lacet) → on tente de **réorienter le bearing**
 *         de façon oblique (45° ± 15° face au versant, jamais 90°) ;
 *       • si la réorientation ne suffit pas → on insère un keyframe N au
 *         point de **moindre courbure** (à mi-distance entre deux virages).
 *   - On réitère sur [A, N] et [N, Z] jusqu'à ce que tout soit visible ou que
 *     la distance minimale entre keyframes (`minKeyframeGapM`) soit atteinte.
 *
 * La visibilité est évaluée à la résolution du viewport de référence
 * (1920×1080), stockée dans le JSON.
 */

import {
  type KeyframeSet,
  type Keyframe,
  type PolyVertex,
  buildTracePolyline,
  buildTracePolylineFromPoints,
  MS_PER_METER,
  DEFAULT_CAM_ZOOM,
  DEFAULT_CAM_PITCH,
} from './keyframeGenerator'
import { bearing, bearingDelta, toRadians } from '../utils/geo'

// --- Paramètres ---

/** Rayon terrestre moyen (m), cohérent avec geo.ts et le backend Rust. */
const EARTH_RADIUS = 6_371_000

/** Champ de vision vertical (deg), défaut Mapbox (~36,87°). */
const VERTICAL_FOV_DEG = 36.87

/** Marge de visibilité : on rétrécit le cadre de vision de ~15 % sur les bords.
 * Le curseur doit rester confortablement dans le cadre (un point à 90 % du bord
 * est perçu comme « hors écran » en lecture réelle, et le rendu Mapbox est
 * légèrement plus pessimiste que le modèle). */
const FOV_MARGIN = 0.85

/** Angle de réorientation oblique face au versant (deg). */
const OBLIQUE_BEARING_DEG = 45
/** Borne basse de la réorientation oblique (deg). */
const OBLIQUE_MIN_DEG = 30
/** Borne haute de la réorientation oblique (deg). */
const OBLIQUE_MAX_DEG = 60

/** Dégagement minimal (m) au-dessus de la ligne de visée pour considérer un
 * point comme occluant (évite le bruit d'altitude GPX). */
const OCCLUSION_CLEARANCE_M = 5

/** Demi-fenêtre (nombre de points) pour la mesure de courbure locale. */
const CURVATURE_WINDOW = 3

/** Pas d'échantillonnage du relief pour le test de ligne de visée (nombre de
 * points). Une crête s'étend sur de nombreux points GPX : on peut espacer le
 * test sans perdre la détection, et éviter un coût O(n²) sur les longues
 * traces. */
const LOS_SAMPLE_STEP = 5

/** Résolution au sol en mètres/pixel à l'équateur au zoom 0 (Web Mercator). */
const MERCATOR_M_PER_PX_EQUATOR = 156543.03392

// --- Types internes ---

/** Point 3D en coordonnées Web Mercator (mètres), z = altitude (m). */
interface WorldPoint {
  x: number
  y: number
  z: number
}

/** Pose de la caméra (position 3D + base orthonormée de vue + focale). */
interface CameraPose {
  x: number
  y: number
  z: number
  bearing: number
  /** Axe « avant » (caméra → centre), unitaire. */
  fx: number
  fy: number
  fz: number
  /** Axe « droite » de l'écran, unitaire. */
  rx: number
  ry: number
  rz: number
  /** Axe « haut » de l'écran, unitaire. */
  ux: number
  uy: number
  uz: number
  /** Focale en pixels (px), identique horizontal/vertical (pixels carrés). */
  focalPx: number
}

/**
 * Échantillonneur d'altitude du terrain (DEM) : renvoie l'altitude (m) du
 * terrain au point (lng, lat), ou `null` si non disponible (tuile non chargée,
 * hors emprise). Fourni par l'appelant (EditionMap) via `queryTerrainElevation`
 * de Mapbox — le module reste ainsi indépendant de Mapbox (spec §3.2).
 */
export type TerrainSampler = (lng: number, lat: number) => number | null

// --- Projection frustum (calcul pur, sans dépendance Mapbox) ---

/** Longitude → coordonnée X Web Mercator (mètres). */
function mercatorX(lng: number): number {
  return EARTH_RADIUS * toRadians(lng)
}

/** Latitude → coordonnée Y Web Mercator (mètres). */
function mercatorY(lat: number): number {
  return EARTH_RADIUS * Math.log(Math.tan(Math.PI / 4 + toRadians(lat) / 2))
}

/** Coordonnées Web Mercator (mètres) → (lng, lat). Inverse de mercatorX/Y. */
function mercatorToLngLat(x: number, y: number): { lng: number; lat: number } {
  const lng = (x / EARTH_RADIUS) * (180 / Math.PI)
  const lat = (2 * Math.atan(Math.exp(y / EARTH_RADIUS)) - Math.PI / 2) * (180 / Math.PI)
  return { lng, lat }
}

/** Convertit un point géographique (lng, lat, altitude) en 3D monde. */
function toWorld(lng: number, lat: number, altitude: number | null): WorldPoint {
  return { x: mercatorX(lng), y: mercatorY(lat), z: altitude ?? 0 }
}

/**
 * Calcule la pose 3D de la caméra centrée sur `center` (lng/lat/altitude) aux
 * paramètres donnés. Le modèle est cohérent avec Mapbox : à un zoom donné
 * correspond une résolution au sol (m/px) ; la distance caméra↔centre découle
 * du FOV vertical et de la hauteur du viewport ; le pitch déplace la caméra en
 * arrière (opposé au bearing) et en hauteur.
 */
function cameraPose(
  centerLng: number,
  centerLat: number,
  centerAltitude: number,
  zoom: number,
  pitch: number,
  bearingDeg: number,
  viewport: { width: number; height: number },
): CameraPose {
  const latRad = toRadians(centerLat)
  // Résolution au sol (m/px) à cette latitude et ce zoom.
  const res = (MERCATOR_M_PER_PX_EQUATOR * Math.cos(latRad)) / Math.pow(2, zoom)
  const vfovRad = toRadians(VERTICAL_FOV_DEG)
  // Focale en pixels puis distance caméra↔centre en mètres.
  const focalPx = viewport.height / 2 / Math.tan(vfovRad / 2)
  const dist = focalPx * res

  const pitchRad = toRadians(pitch)
  const bRad = toRadians(bearingDeg)
  const sinP = Math.sin(pitchRad)
  const cosP = Math.cos(pitchRad)
  const sinB = Math.sin(bRad)
  const cosB = Math.cos(bRad)

  // Position : centre reculé en arrière (opposé au cap) et en hauteur.
  // Repère caméra (convention Mapbox, Mercator x=est / y=nord / z=haut) :
  //   forward (caméra → centre) = (sinB·sinP, cosB·sinP, −cosP)
  //   right   (droite écran)    = (cosB, −sinB, 0)
  //   up      (haut écran)      = (sinB·cosP, cosB·cosP, sinP)
  return {
    x: mercatorX(centerLng) - dist * sinP * sinB,
    y: mercatorY(centerLat) - dist * sinP * cosB,
    z: centerAltitude + dist * cosP,
    bearing: bearingDeg,
    fx: sinB * sinP,
    fy: cosB * sinP,
    fz: -cosP,
    rx: cosB,
    ry: -sinB,
    rz: 0,
    ux: sinB * cosP,
    uy: cosB * cosP,
    uz: sinP,
    focalPx,
  }
}

/**
 * Vérifie si `point` est dans le **champ de vision complet** (horizontal et
 * vertical) de la caméra, par projection perspective dans le repère caméra.
 *
 * Le point est projeté sur le plan image (focale identique horizontal/vertical,
 * pixels carrés) ; il est visible s'il est devant la caméra (profondeur > 0) et
 * si ses coordonnées écran restent dans le cadre, rétréci de `FOV_MARGIN`.
 * Ce test capture à la fois le décalage latéral (lacets) et le décalage
 * vertical (montées/descentes) qui font sortir le curseur du cadre à pitch fort.
 */
function isInFrustum(pose: CameraPose, point: WorldPoint, viewport: { width: number; height: number }): boolean {
  const vx = point.x - pose.x
  const vy = point.y - pose.y
  const vz = point.z - pose.z

  // Profondeur le long de l'axe de visée (négative = derrière la caméra).
  const depth = vx * pose.fx + vy * pose.fy + vz * pose.fz
  if (depth <= 0) return false

  // Coordonnées dans le plan image (caméra), puis en pixels (centre = 0).
  const xCam = vx * pose.rx + vy * pose.ry + vz * pose.rz
  const yCam = vx * pose.ux + vy * pose.uy + vz * pose.uz
  const xPx = (pose.focalPx * xCam) / depth
  const yPx = (pose.focalPx * yCam) / depth

  const halfW = (viewport.width / 2) * FOV_MARGIN
  const halfH = (viewport.height / 2) * FOV_MARGIN
  return Math.abs(xPx) <= halfW && Math.abs(yPx) <= halfH
}

/**
 * Test de ligne de visée : `point` est-il masqué par le **relief** qui coupe le
 * rayon caméra→point en s'élevant au-dessus ?
 *
 * Deux sources, par ordre de priorité :
 *  1. **DEM** (`terrainSampler`, typiquement `queryTerrainElevation` de Mapbox)
 *     — échantillonne l'altitude réelle du terrain le long de la ligne de
 *     visée. Détecte les crêtes **hors de la trace** (versants, massifs) que
 *     les seuls points GPX de la route ne voient pas.
 *  2. **Points de trace intermédiaires** (fallback sans DEM) — approximation
 *     du MNT : la caméra est sur la trace, les points épousent le relief.
 *
 * Un point dont l'altitude dépasse la ligne de visée de plus de
 * `OCCLUSION_CLEARANCE_M` occlut.
 */
function losOccluded(
  camWorld: WorldPoint,
  pointWorld: WorldPoint,
  pointLng: number,
  pointLat: number,
  terrainSampler: TerrainSampler | null,
  world: WorldPoint[],
  lo: number,
  hi: number,
): boolean {
  // 1. Relief réel (DEM) le long de la ligne de visée.
  if (terrainSampler) {
    const byTerrain = losOccludedByTerrain(camWorld, pointWorld, pointLng, pointLat, terrainSampler)
    if (byTerrain !== null) return byTerrain // DEM disponible → fait foi
    // sinon (DEM non chargé) : retomber sur les points de trace ci-dessous.
  }

  // 2. Fallback : points de trace intermédiaires (crêtes sur la route).
  const dx = pointWorld.x - camWorld.x
  const dy = pointWorld.y - camWorld.y
  const dz = pointWorld.z - camWorld.z
  const len2 = dx * dx + dy * dy + dz * dz
  if (len2 < 1e-9) return false

  for (let m = lo; m < hi; m += LOS_SAMPLE_STEP) {
    const t = world[m]
    const rx = t.x - camWorld.x
    const ry = t.y - camWorld.y
    const rz = t.z - camWorld.z
    // Paramètre de projection sur le rayon (0 = caméra, 1 = point cible).
    const s = (rx * dx + ry * dy + rz * dz) / len2
    if (s <= 0 || s >= 1) continue
    const losZ = camWorld.z + s * dz
    if (t.z - losZ > OCCLUSION_CLEARANCE_M) return true
  }
  return false
}

/**
 * Occlusion par le terrain réel (DEM) : échantillonne l'altitude le long de la
 * ligne de visée caméra→point à pas régulier (~100 m, borné) et la compare à la
 * hauteur de la ligne de visée. Retourne `null` si aucune altitude valide n'a
 * pu être échantillonnée (DEM non chargé).
 *
 * La ligne est tracée depuis la **position au sol de la caméra** (déduite de
 * `camWorld`, position 3D de la caméra) — et non depuis le centre de la corde,
 * qui est lui 2,4 km en avant : c'est ce décalage qui faussait l'interpolation
 * de la hauteur de la LOS près du traceur et faisait rater l'occlusion.
 */
function losOccludedByTerrain(
  camWorld: WorldPoint,
  pointWorld: WorldPoint,
  pointLng: number,
  pointLat: number,
  terrainSampler: TerrainSampler,
): boolean | null {
  // Position au sol de la caméra (inverse Web Mercator de sa position 3D).
  const cam = mercatorToLngLat(camWorld.x, camWorld.y)

  // Distance au sol caméra→point (approximation plate, suffisante pour le pas).
  const dLat = pointLat - cam.lat
  const dLng = pointLng - cam.lng
  const mLat = dLat * 111_320
  const mLng = dLng * 111_320 * Math.cos(toRadians((cam.lat + pointLat) / 2))
  const dist2d = Math.sqrt(mLat * mLat + mLng * mLng)

  const step = 50 // m — pas fin pour ne pas rater les buttes côtières étroites
  const samples = Math.max(1, Math.min(200, Math.floor(dist2d / step)))
  let valid = 0
  for (let i = 1; i <= samples; i++) {
    const t = i / (samples + 1)
    const lng = cam.lng + dLng * t
    const lat = cam.lat + dLat * t
    const terr = terrainSampler(lng, lat)
    if (terr == null) continue
    valid++
    const losZ = camWorld.z + (pointWorld.z - camWorld.z) * t
    if (terr - losZ > OCCLUSION_CLEARANCE_M) return true
  }
  return valid > 0 ? false : null
}

// --- Courbure ---

/** Bearing local de la trace à un index (fenêtre de ±1 point). */
function traceBearingAt(poly: PolyVertex[], index: number): number {
  const i0 = Math.max(0, index - 1)
  const i1 = Math.min(poly.length - 1, index + 1)
  return bearing(poly[i0].lat, poly[i0].lng, poly[i1].lat, poly[i1].lng)
}

/** Sinuosité locale = moyenne des |Δbearing| sur une fenêtre autour de `index`.
 * Moyenne (et non somme) pour ne pas pénaliser artificiellement les points
 * proches des bords, où la fenêtre est tronquée. */
function localCurvature(poly: PolyVertex[], index: number): number {
  let total = 0
  let count = 0
  for (let i = index - CURVATURE_WINDOW; i <= index + CURVATURE_WINDOW; i++) {
    if (i <= 0 || i >= poly.length - 1) continue
    const b1 = bearing(poly[i - 1].lat, poly[i - 1].lng, poly[i].lat, poly[i].lng)
    const b2 = bearing(poly[i].lat, poly[i].lng, poly[i + 1].lat, poly[i + 1].lng)
    total += Math.abs(bearingDelta(b1, b2))
    count++
  }
  return count > 0 ? total / count : 0
}

/**
 * Point d'insertion optimal entre `fromIdx` et `toIdx` : celui de **moindre
 * courbure moyenne** (le plus « droit »), à condition d'être au moins à
 * `minGapM` des deux extrémités (sinon le découpage serait rejeté par la
 * garde anti-surabondance). Repli sur l'index médian si aucun point ne
 * respecte cette contrainte.
 */
function bestInsertionPoint(poly: PolyVertex[], fromIdx: number, toIdx: number, minGapM: number): number {
  if (toIdx - fromIdx <= 1) return Math.floor((fromIdx + toIdx) / 2)
  const dFrom = poly[fromIdx].d
  const dTo = poly[toIdx].d
  let bestIdx = -1
  let bestCurv = Infinity
  for (let i = fromIdx + 1; i < toIdx; i++) {
    if (poly[i].d - dFrom < minGapM || dTo - poly[i].d < minGapM) continue
    const c = localCurvature(poly, i)
    if (c < bestCurv) {
      bestCurv = c
      bestIdx = i
    }
  }
  if (bestIdx === -1) return Math.floor((fromIdx + toIdx) / 2)
  return bestIdx
}

// --- Évaluation d'un bearing sur un segment ---

/** Résultat de l'évaluation d'un bearing candidat sur un segment. */
interface SegmentEval {
  /** Nombre de points intermédiaires visibles. */
  visible: number
  /** Nombre total de points intermédiaires du segment. */
  total: number
  /** Nombre de points **dans** le frustum mais masqués par le relief. */
  occluded: number
  /** Premier index de point non visible (ou -1 si tous visibles). */
  firstFailIndex: number
}

/**
 * Compte les points intermédiaires de `[fromIdx, toIdx]` visibles depuis la
 * caméra (qui vole la corde) orientée par `bearingDeg`. Distingue les points
 * sortis du champ (latéral/vertical) des points **masqués par le relief**.
 */
function evaluateBearing(
  poly: PolyVertex[],
  world: WorldPoint[],
  fromIdx: number,
  toIdx: number,
  bearingDeg: number,
  viewport: { width: number; height: number },
  bearingToDeg?: number,
  terrainSampler: TerrainSampler | null = null,
): SegmentEval {
  const a = poly[fromIdx]
  const z = poly[toIdx]
  const total = toIdx - fromIdx - 1
  let visible = 0
  let occluded = 0
  let firstFailIndex = -1

  const totalTrace = z.d - a.d

  for (let k = fromIdx + 1; k < toIdx; k++) {
    const p = poly[k]
    const ratio = totalTrace > 0 ? (p.d - a.d) / totalTrace : 0
    // Position théorique de la caméra sur la corde A→Z.
    const camLng = a.lng + (z.lng - a.lng) * ratio
    const camLat = a.lat + (z.lat - a.lat) * ratio
    const gpxCamAlt = (a.altitude ?? 0) + ((z.altitude ?? 0) - (a.altitude ?? 0)) * ratio
    // Altitude du centre = terrain réel (exagéré) si disponible : la caméra
    // Mapbox est à `terrain(centre) + hauteur`, pas à l'altitude GPX de la route.
    const camAlt = terrainSampler ? (terrainSampler(camLng, camLat) ?? gpxCamAlt) : gpxCamAlt
    // Bearing effectif en lecture : constant si `bearingToDeg` est absent
    // (validation de résolution), sinon interpolé linéairement (chemin le
    // plus court) entre les deux caps des keyframes du segment.
    const b = bearingToDeg === undefined ? bearingDeg : lerpAngle(bearingDeg, bearingToDeg, ratio)
    const pose = cameraPose(camLng, camLat, camAlt, DEFAULT_CAM_ZOOM, DEFAULT_CAM_PITCH, b, viewport)

    // Altitude du traceur = terrain réel si disponible (le marker est posé sur
    // le terrain rendu), sinon altitude GPX.
    const pAlt = terrainSampler ? (terrainSampler(p.lng, p.lat) ?? p.altitude ?? 0) : p.altitude ?? 0
    const pWorld = { x: world[k].x, y: world[k].y, z: pAlt }

    if (!isInFrustum(pose, pWorld, viewport)) {
      if (firstFailIndex === -1) firstFailIndex = k
      continue
    }
    if (losOccluded(pose, pWorld, p.lng, p.lat, terrainSampler, world, fromIdx + 1, k)) {
      occluded++
      if (firstFailIndex === -1) firstFailIndex = k
      continue
    }
    visible++
  }

  return { visible, total, occluded, firstFailIndex }
}

/** Normalise un bearing sur [0, 360). */
function normalizeBearing(deg: number): number {
  return ((deg % 360) + 360) % 360
}

/** Interpolation linéaire d'angles (degrés) avec gestion du wrap 360°. */
function lerpAngle(a: number, b: number, t: number): number {
  const diff = ((b - a + 540) % 360) - 180 // écart le plus court sur [-180, 180]
  return (a + diff * t + 360) % 360
}

/**
 * Résout le bearing d'un segment [fromIdx, toIdx].
 *
 * - Si tout est visible avec le **cap direct** A→Z : fini.
 * - Si des points sont **masqués par le relief** : on tente une réorientation
 *   **oblique** du bearing (30°/45°/60°, des deux côtés) pour faire face au
 *   versant (jamais à 90° de la trace) ; on garde le meilleur candidat.
 * - Si l'échec est purement **latéral/vertical** (point hors du champ) : on ne
 *   change pas de cap et on retourne `allVisible: false` pour déclencher
 *   l'insertion d'un keyframe au point de moindre courbure.
 */
function resolveSegment(
  poly: PolyVertex[],
  world: WorldPoint[],
  fromIdx: number,
  toIdx: number,
  viewport: { width: number; height: number },
  terrainSampler: TerrainSampler | null,
): { bearing: number; allVisible: boolean; firstFailIndex: number } {
  const a = poly[fromIdx]
  const z = poly[toIdx]
  const direct = bearing(a.lat, a.lng, z.lat, z.lng)
  const directEval = evaluateBearing(poly, world, fromIdx, toIdx, direct, viewport, undefined, terrainSampler)

  const allVisible = directEval.total === 0 || directEval.visible === directEval.total
  if (allVisible) return { bearing: direct, allVisible: true, firstFailIndex: -1 }

  // Le relief est en cause : réorientation oblique face au versant.
  if (directEval.occluded > 0) {
    const thetaTrace = traceBearingAt(poly, fromIdx)
    let bestBearing = direct
    let best = directEval
    for (const angle of [OBLIQUE_MIN_DEG, OBLIQUE_BEARING_DEG, OBLIQUE_MAX_DEG]) {
      for (const side of [1, -1]) {
        const cand = normalizeBearing(thetaTrace + side * angle)
        const evalRes = evaluateBearing(poly, world, fromIdx, toIdx, cand, viewport, undefined, terrainSampler)
        if (evalRes.visible > best.visible) {
          best = evalRes
          bestBearing = cand
        }
        if (evalRes.total > 0 && evalRes.visible === evalRes.total) {
          // Tout est visible avec ce candidat : on s'arrête.
          return { bearing: cand, allVisible: true, firstFailIndex: -1 }
        }
      }
    }
    return { bearing: bestBearing, allVisible: false, firstFailIndex: best.firstFailIndex }
  }

  // Échec purement hors champ : on garde le cap direct et on insère.
  return { bearing: direct, allVisible: false, firstFailIndex: directEval.firstFailIndex }
}

// --- Génération récursive ---

/**
 * Génère les keyframes par l'algorithme de frustum (placement récursif par
 * visibilité).
 *
 * @param traceId        - Identifiant de la trace (reporté dans le jeu).
 * @param feature        - Feature GeoJSON LineString de la trace.
 * @param viewport       - Viewport de référence pour la visibilité (1920×1080).
 * @param minKeyframeGapM - Distance minimale entre keyframes (anti-surabondance).
 * @param tracePoints    - Points riches backend (altitude) — priorité si fournis.
 * @param terrainSampler - Échantillonneur d'altitude terrain (DEM) pour
 *                         l'occlusion par le relief ; `null` = fallback sur
 *                         les points de trace.
 * @returns Le jeu de keyframes, ou `null` si la trace est vide.
 */
export function generateFrustumKeyframes(
  traceId: string,
  feature: GeoJSON.Feature,
  viewport: { width: number; height: number },
  minKeyframeGapM: number = 1000,
  tracePoints?: { lat: number; lon: number; alt: number | null; distance_m: number }[] | null,
  terrainSampler?: TerrainSampler | null,
): KeyframeSet | null {
  // 1. Polyligne indexée par distance (avec altitude si points riches fournis).
  let poly: PolyVertex[]
  if (tracePoints && tracePoints.length > 0) {
    poly = buildTracePolylineFromPoints(tracePoints)
  } else {
    poly = buildTracePolyline(feature)
  }
  if (poly.length === 0) return null

  const lastIdx = poly.length - 1
  const totalDistance = poly[lastIdx].d
  const gap = Math.max(1, minKeyframeGapM)
  const sampler = terrainSampler ?? null

  // Points 3D monde précalculés une seule fois (réutilisés par toutes les
  // évaluations de visibilité — évite de les recréer à chaque point).
  const world: WorldPoint[] = poly.map((p) => toWorld(p.lng, p.lat, p.altitude))

  // 2. Placement récursif. `segments` accumule { from, to, bearing }.
  let segments: { from: number; to: number; bearing: number }[] = []

  function place(fromIdx: number, toIdx: number): void {
    const res = resolveSegment(poly, world, fromIdx, toIdx, viewport, sampler)
    segments.push({ from: fromIdx, to: toIdx, bearing: res.bearing })
    if (res.allVisible) return

    const k = bestInsertionPoint(poly, fromIdx, toIdx, gap)
    // Anti-surabondance : ne pas découper en segments plus courts que le gap.
    if (poly[k].d - poly[fromIdx].d < gap || poly[toIdx].d - poly[k].d < gap) return
    place(fromIdx, k)
    place(k, toIdx)
  }

  place(0, lastIdx)

  // 3. Affinage : re-valider chaque segment avec le bearing **interpolé**
  //    réel de la lecture (`lerpAngle` entre les caps des deux keyframes).
  //    En lecture le cap tourne d'un keyframe au suivant ; la résolution
  //    ci-dessus (cap constant) peut laisser des points sortir du cadre.
  //    On découpe alors au point de moindre courbure et on re-résout.
  let guard = 0
  let stabilized = false
  while (!stabilized && guard++ < Math.ceil(totalDistance / gap) + 16) {
    const idxSet = new Set<number>([0, lastIdx])
    for (const s of segments) {
      idxSet.add(s.from)
      idxSet.add(s.to)
    }
    const sortedIdx = [...idxSet].sort((a, b) => poly[a].d - poly[b].d)
    const bearingByFrom = new Map<number, number>()
    for (const s of segments) bearingByFrom.set(s.from, s.bearing)

    stabilized = true
    for (let i = 0; i < sortedIdx.length - 1; i++) {
      const fi = sortedIdx[i]
      const ti = sortedIdx[i + 1]
      // La visibilité prime sur l'espacement minimal : on découpe même les
      // segments courts (jusqu'à `gap/2`) si un point est encore masqué.
      if (poly[ti].d - poly[fi].d < gap) continue // trop court pour découper
      const direct = bearing(poly[fi].lat, poly[fi].lng, poly[ti].lat, poly[ti].lng)
      const bFrom = bearingByFrom.get(fi) ?? direct
      // Le cap d'arrivée est celui du segment suivant ; le dernier keyframe
      // reprend le cap du segment précédent (même logique qu'à l'étape 4).
      const bTo = ti === lastIdx ? bFrom : bearingByFrom.get(ti) ?? bFrom
      const evalRes = evaluateBearing(poly, world, fi, ti, bFrom, viewport, bTo, sampler)
      if (evalRes.total > 0 && evalRes.visible < evalRes.total) {
        const minHalf = gap / 2
        const k = bestInsertionPoint(poly, fi, ti, minHalf)
        if (poly[k].d - poly[fi].d < minHalf || poly[ti].d - poly[k].d < minHalf) continue
        // Remplacer le segment [fi, ti] par deux segments re-résolus.
        const kept = segments.filter((s) => !(s.from === fi && s.to === ti))
        if (kept.length === segments.length) continue // segment absent — on ne boucle pas
        segments = kept
        const res1 = resolveSegment(poly, world, fi, k, viewport, sampler)
        const res2 = resolveSegment(poly, world, k, ti, viewport, sampler)
        segments.push(
          { from: fi, to: k, bearing: res1.bearing },
          { from: k, to: ti, bearing: res2.bearing },
        )
        stabilized = false
        break
      }
    }
  }

  // 4. Collecter les keyframes (index triés/dédoublonnés) depuis les segments.
  const indices = new Set<number>([0, lastIdx])
  for (const s of segments) {
    indices.add(s.from)
    indices.add(s.to)
  }
  const sorted = [...indices].sort((a, b) => poly[a].d - poly[b].d)

  // Bearing de chaque keyframe = bearing du segment dont il est le `from`.
  const bearingByFrom = new Map<number, number>()
  for (const s of segments) bearingByFrom.set(s.from, s.bearing)

  const keyframes: Keyframe[] = sorted.map((idx, i) => {
    const p = poly[idx]
    const isLast = i === sorted.length - 1
    const b = isLast
      ? bearingByFrom.get(sorted[i - 1]) ?? 0
      : bearingByFrom.get(idx) ?? 0
    return {
      time_ms: p.d * MS_PER_METER,
      distance_from_start_m: p.d,
      cam: {
        lng: p.lng,
        lat: p.lat,
        zoom: DEFAULT_CAM_ZOOM,
        bearing: b,
        pitch: DEFAULT_CAM_PITCH,
      },
      traceur: { lng: p.lng, lat: p.lat, altitude: p.altitude },
    }
  })

  return {
    trace_id: traceId,
    total_distance_m: totalDistance,
    total_duration_ms: totalDistance * MS_PER_METER,
    viewport: { ...viewport },
    sample_rate_m: gap,
    keyframes,
  }
}
