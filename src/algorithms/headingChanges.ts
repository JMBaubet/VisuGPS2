/**
 * Analyse des changements de cap (bearing) entre keyframes consécutifs.
 *
 * Module pur (spec §3.2) : aucune dépendance UI. Il calcule, pour chaque paire
 * de keyframes consécutifs, le **virage de cap** effectif que la caméra
 * parcourt en lecture (`bearingDelta` = chemin le plus court, car
 * l'interpolation utilise `lerpAngle`), sa **direction** (horaire /
 * anti-horaire) et son **taux de rotation** (`|Δcap| / Δdist`, en °/km) qui
 * qualifie les changements « brutaux » sur de courtes distances.
 *
 * Fournit aussi l'affichage : la **couleur** porte le sens (teal = horaire,
 * deep-purple = anti-horaire) et **l'épaisseur du trait** porte l'intensité
 * (bandes de 30 °/km : 45, 75, 105, 135…).
 */

import { bearingDelta } from '../utils/geo'
import type { Keyframe } from './keyframeGenerator'

/** Sens de rotation de la caméra entre deux keyframes consécutifs. */
export type RotationDirection = 'horaire' | 'anti-horaire'

/** Un virage de cap entre le keyframe `fromIndex` et le suivant. */
export interface HeadingChange {
  /** Index du keyframe de départ du virage. */
  fromIndex: number
  /** Distance cumulée (m) du keyframe de départ. */
  fromDistanceM: number
  /** Distance cumulée (m) du keyframe d'arrivée. */
  toDistanceM: number
  /** Longueur du segment (m). */
  distM: number
  /** Delta de cap signé (degrés) : ≥ 0 = horaire, < 0 = anti-horaire. */
  deltaDeg: number
  /** Taux de rotation : |Δcap| / (Δdist en km), en °/km. */
  rateDegPerKm: number
  /** Sens de rotation. */
  direction: RotationDirection
}

/** Teintes de base par sens de rotation (teal = horaire, deep-purple =
 * anti-horaire). La **couleur** ne porte que le sens ; **l'intensité** du virage
 * est portée par l'**épaisseur** du trait (voir `headingChangeThickness`). */
export const ROTATION_BASE_COLORS: Record<RotationDirection, string> = {
  horaire: '#009688', // teal
  'anti-horaire': '#673AB7', // deep-purple
}

/** Pas des bandes d'épaisseur (degrés/km) : à seuil 45 → 45, 75, 105, 135, 165… */
const RATE_BAND_STEP_DEG_PER_KM = 30

/** Épaisseur du trait sous le seuil (px). */
const THICKNESS_SUB = 2

/** Incrément d'épaisseur par bande (px). */
const THICKNESS_STEP = 2

/**
 * Calcule les changements de cap entre keyframes consécutifs.
 *
 * Le virage effectif en lecture est le delta **signé le plus court**
 * (`bearingDelta`) : la caméra ne tourne jamais de plus de 180° entre deux
 * keyframes (interpolation `lerpAngle`). Les segments de longueur nulle sont
 * ignorés.
 */
export function computeHeadingChanges(keyframes: Keyframe[]): HeadingChange[] {
  const out: HeadingChange[] = []
  for (let i = 0; i < keyframes.length - 1; i++) {
    const a = keyframes[i]
    const b = keyframes[i + 1]
    const distM = b.distance_from_start_m - a.distance_from_start_m
    if (distM <= 0) continue
    const deltaDeg = bearingDelta(a.cam.bearing, b.cam.bearing)
    out.push({
      fromIndex: i,
      fromDistanceM: a.distance_from_start_m,
      toDistanceM: b.distance_from_start_m,
      distM,
      deltaDeg,
      rateDegPerKm: Math.abs(deltaDeg) / (distM / 1000),
      direction: deltaDeg >= 0 ? 'horaire' : 'anti-horaire',
    })
  }
  return out
}

/**
 * Épaisseur (px) du trait d'un virage sur la timeline, selon son taux de
 * rotation.
 *
 * Sous le seuil : trait fin fixe (`THICKNESS_SUB`). Au-dessus : chaque bande de
 * `RATE_BAND_STEP_DEG_PER_KM` (30 °/km — à seuil 45 : 45, 75, 105, 135, 165…)
 * ajoute `THICKNESS_STEP` pixels. Plus le changement de cap est important, plus
 * le trait est épais.
 */
export function headingChangeThickness(
  rateDegPerKm: number,
  thresholdDegPerKm: number,
): number {
  if (rateDegPerKm < thresholdDegPerKm) return THICKNESS_SUB
  const band =
    Math.floor((rateDegPerKm - thresholdDegPerKm) / RATE_BAND_STEP_DEG_PER_KM) + 1
  return THICKNESS_SUB + band * THICKNESS_STEP
}
