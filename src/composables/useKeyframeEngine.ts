/**
 * Composable du moteur de keyframes (Modes 1 & 2).
 *
 * Regroupe la logique non-réactive pure du pré-calcul et de la lecture, qui
 * dépend d'une instance `mapboxgl.Map` :
 *
 *  - `precomputeKeyframes(map, coords, opts)` : génère les keyframes bruts
 *    (algorithme de zone morte + altitudes `queryTerrainElevation`), Mode 1.
 *  - `computeCamAt(map, raw, time)` : interpole l'état caméra/traceur à un
 *    instant `time` (LERP position, SLERP bearing), utilisé par le live preview.
 *
 * La fusion (blending) des overrides, qui ne dépend pas d'une map, est
 * implémentée ici aussi (`blendKeyframes`) pour centraliser la logique de
 * montage, bien qu'elle soit pure (réutilisable hors d'une instance Mapbox).
 *
 * Convention : `coords` est au format Mapbox `[lon, lat][]` (LineString).
 */

import type { Map as MbMap } from 'mapbox-gl'
import { haversineMeters } from '../utils/geo'
import { lerp, slerpAngle, easingFn } from '../utils/easing'
import type {
  RawKeyframesFile,
  Keyframe,
  CamState,
  TraceurState,
  Override,
  ReferenceViewport,
} from '../utils/keyframes'

// ---------------------------------------------------------------------------
// Paramètres du pré-calcul (lus depuis les settings du backend)
// ---------------------------------------------------------------------------

/** Tous les paramètres nécessaires au pré-calcul, groupés par catégorie. */
export interface PrecomputeOptions {
  viewport: ReferenceViewport
  sampleRate: number
  cam: { zoom: number; pitch: number; bearing: number }
  deadZone: { x: number; y: number; anticipationTime: number; bearingSmoothThreshold: number }
  flyTo: {
    minDuration: number
    maxDuration: number
    speedFactor: number
    zoomAltitudeFactor: number
  }
}

/**
 * Valeurs par défaut des options de pré-calcul.
 * Utilisées si la lecture d'un paramètre backend échoue silencieusement
 * (ex. settings non chargés). Reflète le `settings.default.toml`.
 */
export const DEFAULT_PRECOMPUTE_OPTIONS: PrecomputeOptions = {
  viewport: { width: 1920, height: 1080 },
  sampleRate: 100,
  cam: { zoom: 16, pitch: 60, bearing: 0 },
  deadZone: { x: 0.30, y: 0.30, anticipationTime: 1.5, bearingSmoothThreshold: 15 },
  flyTo: { minDuration: 800, maxDuration: 12000, speedFactor: 4, zoomAltitudeFactor: 0.5 },
}

// ---------------------------------------------------------------------------
// Échantillonnage de la trace
// ---------------------------------------------------------------------------

/**
 * Point échantillonné le long de la trace : position géographique et distance
 * cumulée depuis le départ (en mètres). Interne au pré-calcul.
 */
interface SamplePoint {
  lng: number
  lat: number
  distance: number // distance cumulée depuis le départ (m)
}

/**
 * Calcule la distance cumulée le long d'une LineString `[lon, lat][]`.
 * @returns tableau de distances, `distances[i]` étant la distance cumulée
 *          au point `coords[i]` (en mètres). `distances[0] = 0`.
 */
function cumulativeDistances(coords: [number, number][]): number[] {
  const distances = new Array<number>(coords.length)
  distances[0] = 0
  for (let i = 1; i < coords.length; i++) {
    const [lon1, lat1] = coords[i - 1]
    const [lon2, lat2] = coords[i]
    distances[i] = distances[i - 1] + haversineMeters(lat1, lon1, lat2, lon2)
  }
  return distances
}

/**
 * Échantillonne la trace à intervalles temporels réguliers (`sampleRate` ms).
 *
 * La position du traceur est interpolée linéairement entre les points GPX en
 * fonction de la distance parcourue (vitesse uniforme imposée par
 * `speedFactor` ms/m, soit 4000 ms/km par défaut). Le temps total dépend donc
 * de la distance totale, conformément à la spécification (§1 : 4000 ms/km).
 *
 * @param coords  - LineString `[lon, lat][]`.
 * @param sampleRate - Fréquence d'échantillonnage (ms).
 * @param speedFactor - ms par mètre (ex. 4 → 4000 ms/km).
 * @returns `{ samples, totalDuration, totalDistance }`.
 */
function sampleTrace(
  coords: [number, number][],
  sampleRate: number,
  speedFactor: number,
): { samples: SamplePoint[]; totalDuration: number; totalDistance: number } {
  const distances = cumulativeDistances(coords)
  const totalDistance = distances[distances.length - 1]
  // Durée totale : temps pour parcourir la distance totale à speedFactor ms/m.
  const totalDuration = Math.max(totalDistance * speedFactor, sampleRate)

  const samples: SamplePoint[] = []
  let segIdx = 0 // segment courant (entre coords[segIdx] et coords[segIdx+1])

  for (let t = 0; t <= totalDuration; t += sampleRate) {
    const targetDistance = (t / totalDuration) * totalDistance
    // Avancer segIdx jusqu'au segment contenant targetDistance.
    while (
      segIdx < coords.length - 2 &&
      distances[segIdx + 1] < targetDistance
    ) {
      segIdx++
    }
    const segStart = distances[segIdx]
    const segEnd = distances[segIdx + 1] ?? segStart
    const segLen = segEnd - segStart || 1
    const f = Math.max(0, Math.min(1, (targetDistance - segStart) / segLen))

    const [lon1, lat1] = coords[segIdx]
    const [lon2, lat2] = coords[Math.min(segIdx + 1, coords.length - 1)]
    samples.push({
      lng: lerp(lon1, lon2, f),
      lat: lerp(lat1, lat2, f),
      distance: targetDistance,
    })
  }

  // Garantir que le dernier point correspond à la fin de la trace.
  const lastCoord = coords[coords.length - 1]
  if (
    samples.length === 0 ||
    samples[samples.length - 1].distance < totalDistance - 1e-6
  ) {
    samples.push({ lng: lastCoord[0], lat: lastCoord[1], distance: totalDistance })
  }

  return { samples, totalDuration, totalDistance }
}

// ---------------------------------------------------------------------------
// Altitudes via queryTerrainElevation (lots de 50, délai 100 ms)
// ---------------------------------------------------------------------------

/**
 * Promesse résolue après `ms` millisecondes (utilisée pour respecter les
 * limites de taux de l'API terrain entre les lots).
 */
function delay(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms))
}

/**
 * Recherche l'échantillon le plus proche d'un instant futur (anticipation).
 *
 * Utilisé pour le calcul du bearing : on regarde vers la position du traceur
 * `anticipationTime` secondes plus tard, afin d'anticiper les virages.
 */
function sampleAtTime(
  samples: SamplePoint[],
  time: number,
  totalDuration: number,
): SamplePoint {
  const idx = Math.min(
    samples.length - 1,
    Math.max(0, Math.round((time / totalDuration) * (samples.length - 1))),
  )
  return samples[idx]
}

/**
 * Interroge `map.queryTerrainElevation` pour tous les échantillons, par lots
 * de 50 avec un délai de 100 ms entre chaque lot (§8 : respect des limites de
 * taux Mapbox). Source primaire Mapbox ; `null` en cas d'échec (repli non
 * disponible ici : la LineString 2D ne contient pas l'altitude GPX).
 *
 * @param onProgress - callback de progression (0 → 1) pour l'overlay UI.
 */
async function queryAltitudes(
  map: MbMap,
  samples: SamplePoint[],
  onProgress: (fraction: number) => void,
): Promise<(number | null)[]> {
  const altitudes: (number | null)[] = new Array(samples.length).fill(null)
  const batchSize = 50

  for (let i = 0; i < samples.length; i += batchSize) {
    const end = Math.min(i + batchSize, samples.length)
    for (let j = i; j < end; j++) {
      const elev = map.queryTerrainElevation([samples[j].lng, samples[j].lat])
      altitudes[j] = elev ?? null
    }
    onProgress(end / samples.length)
    if (end < samples.length) {
      await delay(100)
    }
  }

  return altitudes
}

// ---------------------------------------------------------------------------
// Algorithme de zone morte (Dead Zone)
// ---------------------------------------------------------------------------

/**
 * Calcule les keyframes (caméra + traceur) à partir des échantillons et des
 * altitudes, en appliquant l'algorithme de zone morte (§3 du document).
 *
 * Principe : la position du traceur est projetée à l'écran (`map.project`).
 * Tant qu'elle reste dans une zone centrale (deadZoneX/Y du viewport de
 * référence), la caméra ne bouge pas. Dès qu'elle s'en approche des bords, un
 * flyTo recentre la caméra sur le traceur. Le bearing est calculé par
 * anticipation et lissé.
 *
 * NOTE : cette fonction modifie le centre/zoom de la map fournie (elle
 * déplace la caméra pour pouvoir projeter les positions futures). L'appelant
 * est responsable d'avoir forcé la résolution canonique au préalable.
 */
function buildKeyframesFromSamples(
  map: MbMap,
  samples: SamplePoint[],
  altitudes: (number | null)[],
  totalDuration: number,
  opts: PrecomputeOptions,
  onProgress: (fraction: number) => void,
): Keyframe[] {
  const {
    viewport,
    cam,
    deadZone,
    flyTo,
  } = opts

  const deadZonePxX = (deadZone.x * viewport.width) / 2
  const deadZonePxY = (deadZone.y * viewport.height) / 2

  const keyframes: Keyframe[] = []
  // Position caméra courante (celle rendue dans le dernier keyframe).
  let camLng = samples[0].lng
  let camLat = samples[0].lat
  let camBearing = cam.bearing

  // État du vol (flyTo) en cours. Quand la caméra doit se rattraper vers une
  // nouvelle position, on interpole linéairement sa position sur plusieurs
  // keyframes au lieu de la téléporter en un seul pas (cause des saccades).
  // `flyStart`/`flyEnd` sont en millisecondes ; `flyFrom*`/`flyTo*` en coords.
  let flyFromLng = camLng
  let flyFromLat = camLat
  let flyFromBearing = camBearing
  let flyToLng = camLng
  let flyToLat = camLat
  let flyToBearing = camBearing
  let flyStart = 0
  let flyEnd = 0 // 0 == aucun vol en cours
  // Index du sample auquel le vol courant a été déclenché (pour calculer la
  // distance réelle le long de la trace, et non à vol d'oiseau).
  let flyFromSampleIdx = 0

  for (let i = 0; i < samples.length; i++) {
    const sample = samples[i]
    const time = Math.round((i / (samples.length - 1)) * totalDuration)
    const altitude = altitudes[i]

    // zoom ajusté par altitude pour éviter de traverser le relief.
    const zoom =
      cam.zoom + (altitude ? (altitude / 1000) * flyTo.zoomAltitudeFactor : 0)

    // Projeter la position du traceur avec la caméra courante.
    map.jumpTo({
      center: [camLng, camLat],
      zoom,
      bearing: camBearing,
      pitch: cam.pitch,
    })
    const projected = map.project([sample.lng, sample.lat])
    const offsetX = projected.x - viewport.width / 2
    const offsetY = projected.y - viewport.height / 2

    // Zone morte : si le traceur sort de la zone centrale ET qu'aucun vol
    // n'est en cours, déclencher un nouveau vol de rattrapage vers le traceur.
    const outOfDeadZone =
      Math.abs(offsetX) > deadZonePxX || Math.abs(offsetY) > deadZonePxY
    if (outOfDeadZone && time >= flyEnd) {
      // Distance réelle parcourue le long de la trace depuis le déclenchement
      // du vol précédent (différence des distances cumulées des samples, et non
      // distance à vol d'oiseau entre les deux positions caméra).
      const distanceM = Math.max(0, sample.distance - samples[flyFromSampleIdx].distance)
      // Durée du vol proportionnelle à la distance, bornée [min, max] (§5.3).
      const duration = Math.max(
        flyTo.minDuration,
        Math.min(distanceM * flyTo.speedFactor, flyTo.maxDuration),
      )

      // Cap anticipé : direction vers la position future du traceur.
      const future = sampleAtTime(
        samples,
        Math.min(time + deadZone.anticipationTime * 1000, totalDuration),
        totalDuration,
      )
      const anticipatedBearing = bearingBetween(
        sample.lng,
        sample.lat,
        future.lng,
        future.lat,
      )

      flyFromLng = camLng
      flyFromLat = camLat
      flyFromBearing = camBearing
      flyToLng = sample.lng
      flyToLat = sample.lat
      // Lissage du cap : ne changer la cible que si l'écart dépasse le seuil.
      flyToBearing =
        angularDiff(camBearing, anticipatedBearing) > deadZone.bearingSmoothThreshold
          ? anticipatedBearing
          : camBearing
      flyStart = time
      flyEnd = time + duration
      flyFromSampleIdx = i
    }

    // Si un vol est en cours, interpoler la position caméra sur ce pas.
    if (flyEnd > time) {
      const span = Math.max(flyEnd - flyStart, 1)
      const t = Math.max(0, Math.min(1, (time - flyStart) / span))
      camLng = lerp(flyFromLng, flyToLng, t)
      camLat = lerp(flyFromLat, flyToLat, t)
      camBearing = slerpAngle(flyFromBearing, flyToBearing, t)
    }

    keyframes.push({
      time,
      cam: {
        lng: camLng,
        lat: camLat,
        zoom,
        bearing: camBearing,
        pitch: cam.pitch,
      },
      traceur: {
        lng: sample.lng,
        lat: sample.lat,
        altitude,
      },
    })

    onProgress((i + 1) / samples.length)
  }

  return keyframes
}

/** Cap (bearing) du segment [a → b], en degrés [0, 360[. */
function bearingBetween(aLng: number, aLat: number, bLng: number, bLat: number): number {
  const toRad = Math.PI / 180
  const toDeg = 180 / Math.PI
  const φ1 = aLat * toRad
  const φ2 = bLat * toRad
  const Δλ = (bLng - aLng) * toRad
  const y = Math.sin(Δλ) * Math.cos(φ2)
  const x =
    Math.cos(φ1) * Math.sin(φ2) -
    Math.sin(φ1) * Math.cos(φ2) * Math.cos(Δλ)
  return (Math.atan2(y, x) * toDeg + 360) % 360
}

/** Écart angulaire absolu minimal entre deux caps (degrés, [0, 180]). */
function angularDiff(a: number, b: number): number {
  const diff = Math.abs(a - b) % 360
  return diff > 180 ? 360 - diff : diff
}

// ---------------------------------------------------------------------------
// Fonction principale : pré-calcul complet
// ---------------------------------------------------------------------------

/** Callbacks de progression découpées par phase (affichées dans l'overlay). */
export interface PrecomputeCallbacks {
  onPhase?: (phase: 'altitudes' | 'keyframes' | 'done', label: string) => void
  onProgress?: (fraction: number) => void
}

/**
 * Lance le pré-calcul complet des keyframes d'une trace (Mode 1).
 *
 * Étapes :
 *  1. Forcer la résolution canonique (`map.resize`).
 *  2. Échantillonner la trace à `sampleRate`.
 *  3. Relever les altitudes via `queryTerrainElevation` (lots de 50, 100 ms).
 *  4. Calculer les keyframes avec l'algorithme de zone morte.
 *
 * @param map     - instance Mapbox avec terrain chargé (le caller garantit
 *                  que le DEM est prêt avant l'appel).
 * @param coords  - LineString `[lon, lat][]` de la trace.
 * @param traceId - identifiant de la trace (stocké dans le fichier de sortie).
 * @param opts    - paramètres de pré-calcul.
 * @param callbacks - progression par phase (UI).
 * @returns le fichier `RawKeyframesFile` prêt à être sauvegardé.
 */
export async function precomputeKeyframes(
  map: MbMap,
  coords: [number, number][],
  traceId: string,
  opts: PrecomputeOptions,
  callbacks: PrecomputeCallbacks = {},
): Promise<RawKeyframesFile> {
  const { viewport, sampleRate, flyTo } = opts

  // 1. Forcer la résolution canonique (indépendance vis-à-vis de l'écran réel).
  map.resize()
  map.getContainer().style.width = `${viewport.width}px`
  map.getContainer().style.height = `${viewport.height}px`
  map.resize()

  // 2. Échantillonner la trace.
  const { samples, totalDuration, totalDistance } = sampleTrace(
    coords,
    sampleRate,
    flyTo.speedFactor,
  )

  // 3. Altitudes (lots de 50, délai 100 ms).
  callbacks.onPhase?.('altitudes', `Calcul des altitudes (${samples.length} points)…`)
  const altitudes = await queryAltitudes(map, samples, (f) =>
    callbacks.onProgress?.(0.5 * f),
  )

  // 4. Keyframes (zone morte).
  callbacks.onPhase?.('keyframes', 'Simulation de la caméra (zone morte)…')
  const keyframes = buildKeyframesFromSamples(
    map,
    samples,
    altitudes,
    totalDuration,
    opts,
    (f) => callbacks.onProgress?.(0.5 + 0.5 * f),
  )

  callbacks.onPhase?.('done', 'Terminé')

  return {
    trace_id: traceId,
    total_duration: Math.round(totalDuration),
    total_distance: totalDistance,
    reference_viewport: viewport,
    sample_rate: sampleRate,
    keyframes,
  }
}

// ---------------------------------------------------------------------------
// Lecture : interpolation d'état à un instant donné (live preview)
// ---------------------------------------------------------------------------

/** État interpolé à un instant `time` (utilisé par le live preview). */
export interface InterpolatedState {
  cam: CamState
  traceur: TraceurState
}

/**
 * Calcule l'état caméra + traceur à un instant `time`, par interpolation
 * linéaire (LERP pour position/zoom/pitch, SLERP pour le bearing) entre les
 * deux keyframes encadrants.
 *
 * Gère le hors-plage : avant le premier keyframe → premier keyframe ; après le
 * dernier → dernier keyframe.
 */
export function computeCamAt(raw: RawKeyframesFile, time: number): InterpolatedState {
  const kf = raw.keyframes
  if (kf.length === 0) {
    throw new Error('Aucun keyframe à interpoler (fichier brut vide).')
  }
  if (time <= kf[0].time) return { cam: kf[0].cam, traceur: kf[0].traceur }
  const last = kf[kf.length - 1]
  if (time >= last.time) return { cam: last.cam, traceur: last.traceur }

  // Recherche dichotomique du segment encadrant [lo, hi].
  let lo = 0
  let hi = kf.length - 1
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1
    if (kf[mid].time <= time) lo = mid
    else hi = mid
  }
  const a = kf[lo]
  const b = kf[hi]
  const span = b.time - a.time || 1
  const t = Math.max(0, Math.min(1, (time - a.time) / span))

  return {
    cam: {
      lng: lerp(a.cam.lng, b.cam.lng, t),
      lat: lerp(a.cam.lat, b.cam.lat, t),
      zoom: lerp(a.cam.zoom, b.cam.zoom, t),
      bearing: slerpAngle(a.cam.bearing, b.cam.bearing, t),
      pitch: lerp(a.cam.pitch, b.cam.pitch, t),
    },
    traceur: {
      lng: lerp(a.traceur.lng, b.traceur.lng, t),
      lat: lerp(a.traceur.lat, b.traceur.lat, t),
      altitude:
        a.traceur.altitude != null && b.traceur.altitude != null
          ? lerp(a.traceur.altitude, b.traceur.altitude, t)
          : a.traceur.altitude ?? b.traceur.altitude,
    },
  }
}

// ---------------------------------------------------------------------------
// Fusion (Blending) — applique les overrides caméra aux keyframes bruts.
//
// NOTE : sera étendue en M3 (lissage des bords de plage par easing). La
// signature est stable ; cette version de base applique les valeurs absolues
// et offsets sur les keyframes dont `time` est dans une plage d'override.
// ---------------------------------------------------------------------------

/**
 * Applique un override à un état caméra (valeur de base) pour un facteur
 * d'application `blend` ∈ [0, 1] (1 = override pleinement appliqué, 0 = brut).
 *
 * Les valeurs absolues (`zoom`, `pitch`, `bearing`) remplacent la valeur
 * brute ; les offsets relatifs (`*_offset`) s'y ajoutent. Le facteur `blend`
 * permet de fondre progressivement entre la valeur brute et la valeur override
 * (lissage des bords de plage). Le centre (lng/lat) n'est jamais touché (§6.2).
 */
function applyOverrideToCam(cam: CamState, ov: Override, blend: number): CamState {
  const p = ov.params
  let zoom = cam.zoom
  let pitch = cam.pitch
  let bearing = cam.bearing

  if (p.zoom !== undefined) zoom = lerp(cam.zoom, p.zoom, blend)
  if (p.zoom_offset !== undefined) zoom += p.zoom_offset * blend
  if (p.pitch !== undefined) pitch = lerp(cam.pitch, p.pitch, blend)
  if (p.pitch_offset !== undefined) pitch += p.pitch_offset * blend
  if (p.bearing !== undefined) bearing = slerpAngle(cam.bearing, p.bearing, blend)
  if (p.bearing_offset !== undefined) {
    bearing = slerpAngle(bearing, bearing + p.bearing_offset, blend)
  }

  return { lng: cam.lng, lat: cam.lat, zoom, pitch, bearing }
}

/** Fenêtre de lissage par défaut aux bords d'une plage d'override (ms). */
const EASE_WINDOW_MS = 500

/**
 * Calcule le facteur d'application d'un override à l'instant `time`.
 *
 * - En dehors de la plage → 0.
 * - Au cœur de la plage (hors fenêtre de lissage) → 1.
 * - Aux bords (entrée/sortie) → transition 0↔1 via la fonction d'easing de
 *   l'override. Le `damping` (0..1) réduit l'amplitude maximale (effet d'inertie).
 */
function overrideBlendFactor(ov: Override, time: number): number {
  if (time < ov.start_time || time > ov.end_time) return 0
  const span = Math.max(ov.end_time - ov.start_time, 1)
  const window = Math.min(EASE_WINDOW_MS, span / 2)
  const ease = easingFn(ov.easing)
  const damping = ov.damping !== undefined ? Math.max(0, Math.min(1, ov.damping)) : 1
  const amplitude = damping

  let t: number
  if (time < ov.start_time + window) {
    // Entrée : 0 → amplitude.
    t = (time - ov.start_time) / window
    return ease(t) * amplitude
  }
  if (time > ov.end_time - window) {
    // Sortie : amplitude → 0.
    t = (ov.end_time - time) / window
    return ease(t) * amplitude
  }
  // Cœur : pleinement appliqué.
  return amplitude
}

/**
 * Fusionne les keyframes bruts avec les overrides actifs (moteur de fusion).
 *
 * Pour chaque keyframe, on calcule la contribution de chaque override actif
 * via son facteur d'application (lissé aux bords par easing + damping). Les
 * overrides successifs se composent par interpolation successive (le dernier
 * appliqué a le dernier mot sur les champs qu'il définit).
 *
 * Le `center` (lng/lat) reste issu de l'algorithme de zone morte (jamais
 * surchargé, §6.2).
 *
 * NOTE : `speed_multiplier` n'est pas propagé dans le keyframe final à ce
 * stade (le schéma `Keyframe` ne porte pas encore ce champ). Il sera utilisé
 * en M5 directement par la boucle de lecture lors de la traversal.
 */
export function blendKeyframes(
  raw: RawKeyframesFile,
  overrides: Override[],
): RawKeyframesFile {
  const active = overrides.filter(o => !o.disabled)

  const blended: Keyframe[] = raw.keyframes.map((kf) => {
    let cam = kf.cam
    for (const ov of active) {
      const blend = overrideBlendFactor(ov, kf.time)
      if (blend <= 0) continue
      cam = applyOverrideToCam(cam, ov, blend)
    }
    return { time: kf.time, cam, traceur: kf.traceur }
  })

  return { ...raw, keyframes: blended }
}

// ---------------------------------------------------------------------------
// Helper : charge les options depuis le backend (settings)
// ---------------------------------------------------------------------------

/**
 * Lit les options de pré-calcul depuis les paramètres du backend.
 *
 * Toute erreur de lecture (paramètre absent, backend indisponible) retombe
 * silencieusement sur la valeur par défaut correspondante, afin de ne jamais
 * bloquer le pré-calcul.
 */
export async function loadPrecomputeOptions(
  getSettingValue: (path: string) => Promise<unknown>,
): Promise<PrecomputeOptions> {
  const get = async <T,>(path: string, fallback: T): Promise<T> => {
    try {
      const v = await getSettingValue(path)
      return (v ?? fallback) as T
    } catch {
      return fallback
    }
  }

  const opts = DEFAULT_PRECOMPUTE_OPTIONS
  return {
    viewport: {
      width: await get('EditionCamera.Viewport.largeurReference', opts.viewport.width),
      height: await get('EditionCamera.Viewport.hauteurReference', opts.viewport.height),
    },
    sampleRate: await get('EditionCamera.PreCalcul.sampleRate', opts.sampleRate),
    cam: {
      zoom: await get('EditionCamera.Cam.defaultZoom', opts.cam.zoom),
      pitch: await get('EditionCamera.Cam.defaultPitch', opts.cam.pitch),
      bearing: await get('EditionCamera.Cam.defaultBearing', opts.cam.bearing),
    },
    deadZone: {
      x: await get('EditionCamera.ZoneMorte.deadZoneX', opts.deadZone.x),
      y: await get('EditionCamera.ZoneMorte.deadZoneY', opts.deadZone.y),
      anticipationTime: await get('EditionCamera.ZoneMorte.anticipationTime', opts.deadZone.anticipationTime),
      bearingSmoothThreshold: await get('EditionCamera.ZoneMorte.bearingSmoothThreshold', opts.deadZone.bearingSmoothThreshold),
    },
    flyTo: {
      minDuration: await get('EditionCamera.FlyTo.minFlyDuration', opts.flyTo.minDuration),
      maxDuration: await get('EditionCamera.FlyTo.maxFlyDuration', opts.flyTo.maxDuration),
      speedFactor: await get('EditionCamera.FlyTo.speedFactor', opts.flyTo.speedFactor),
      zoomAltitudeFactor: await get('EditionCamera.FlyTo.zoomAltitudeFactor', opts.flyTo.zoomAltitudeFactor),
    },
  }
}
