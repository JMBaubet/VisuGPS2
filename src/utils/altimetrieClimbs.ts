/**
 * Helpers purs de manipulation des montées.
 *
 * - `constrainClimbBound` applique les contraintes de déplacement du prototype :
 *   pas de chevauchement (1 m entre montées) et longueur minimale de 50 m.
 * - `climbToEdit` projette l'état persisté vers la forme envoyée à
 *   `altimetrie_preview` / `altimetrie_save` ; le backend recalcule toutes les
 *   métriques et ne fait jamais confiance aux valeurs dérivées.
 */

import type {
  AltimetrieClimb,
  AltimetrieClimbEdit,
  AltimetrieProfilePoint,
} from '../stores/altimetrie'

export type ClimbHandle = 'valley' | 'summit'

/** Distance minimale entre deux montées (km) — 1 m. */
const MIN_GAP_KM = 0.001
/** Longueur minimale d'une montée (km) — 50 m. */
export const MIN_CLIMB_LENGTH_KM = 0.05

/**
 * Contraint une borne de montée.
 *
 * La montée est repérée par son identifiant dans la liste **triée par
 * `startKm`** ; ses voisines immédiates bornent le déplacement. Une borne de
 * vallée ne peut pas dépasser le sommet moins 50 m ; une borne de sommet ne
 * peut pas précéder la vallée plus 50 m.
 */
export function constrainClimbBound(
  climbs: AltimetrieClimb[],
  climbId: number,
  handle: ClimbHandle,
  km: number,
  totalKm: number,
): number {
  const sorted = [...climbs].sort((a, b) => a.startKm - b.startKm)
  const index = sorted.findIndex(climb => climb.id === climbId)
  if (index === -1) return km

  const climb = sorted[index]
  const previous = index > 0 ? sorted[index - 1] : null
  const next = index < sorted.length - 1 ? sorted[index + 1] : null
  const minKm = previous ? previous.endKm + MIN_GAP_KM : 0
  const maxKm = next ? next.startKm - MIN_GAP_KM : totalKm

  if (handle === 'valley') {
    return Math.max(minKm, Math.min(climb.endKm - MIN_CLIMB_LENGTH_KM, km))
  }
  return Math.max(climb.startKm + MIN_CLIMB_LENGTH_KM, Math.min(maxKm, km))
}

/**
 * Applique une borne déplacée à une montée, pour l'aperçu pendant le glissement.
 *
 * Les métriques sont recalculées depuis le profil affiché : longueur, D+
 * (somme des dénivelées positives), pentes et altitudes de début/fin. Le
 * backend revalide et arrondit les bornes au relâchement ; cet aperçu ne sert
 * qu'à faire réagir la carte et la liste en temps réel.
 */
export function applyClimbBounds(
  climb: AltimetrieClimb,
  startKm: number,
  endKm: number,
  profile: AltimetrieProfilePoint[],
  totalDistanceM: number,
): AltimetrieClimb {
  const first = profile[0]
  const last = profile[profile.length - 1]
  if (!first || !last) return climb

  const minKm = first.d / 1000
  const maxKm = last.d / 1000
  const start = Math.max(minKm, Math.min(maxKm, Math.min(startKm, endKm)))
  const end = Math.max(minKm, Math.min(maxKm, Math.max(startKm, endKm)))
  const lengthM = (end - start) * 1000
  if (lengthM <= 0) return climb

  const startM = start * 1000
  const endM = end * 1000
  const dPlus = positiveElevationGain(profile, startM, endM)
  const totalM = totalDistanceM > 0 ? totalDistanceM : last.d

  return {
    ...climb,
    startKm: round(start, 4),
    endKm: round(end, 4),
    startProgress: round(startM / totalM, 5),
    endProgress: round(endM / totalM, 5),
    dPlusM: Math.round(dPlus),
    avgSlopePct: round((dPlus / lengthM) * 100, 2),
    altStartM: round(elevationAt(profile, startM), 1),
    altEndM: round(elevationAt(profile, endM), 1),
  }
}

/** Métriques du segment minimal contenant une distance. */
export interface MinimalSegmentMetrics {
  distanceM: number
  slopePct: number
  altitudeM: number
}

/**
 * Pente et altitude du segment minimal contenant une distance.
 *
 * Le segment minimal est l'intervalle entre deux points consécutifs du profil
 * affiché. La pente est calculée entre ses deux extrémités ; l'altitude
 * retournée est la moyenne des altitudes de ces deux extrémités.
 */
export function minimalSegmentAt(
  profile: AltimetrieProfilePoint[],
  distanceM: number,
): MinimalSegmentMetrics | null {
  const first = profile[0]
  const last = profile[profile.length - 1]
  if (!first || !last || profile.length < 2) return null

  const target = Math.max(first.d, Math.min(last.d, distanceM))
  let index = lowerBound(profile, target)
  if (index <= 0) index = 1

  const previous = profile[index - 1]
  const next = profile[index] ?? last
  if (!previous || !next || next.d <= previous.d) return null

  return {
    distanceM: target,
    slopePct: ((next.ele - previous.ele) / (next.d - previous.d)) * 100,
    altitudeM: (previous.ele + next.ele) / 2,
  }
}

/** Altitude interpolée du profil à une distance donnée (m). */
function elevationAt(
  profile: AltimetrieProfilePoint[],
  targetM: number,
): number {
  const first = profile[0]
  const last = profile[profile.length - 1]
  if (!first || !last) return 0
  if (targetM <= first.d) return first.ele
  if (targetM >= last.d) return last.ele

  const index = lowerBound(profile, targetM)
  const previous = profile[index - 1]
  const next = profile[index]
  if (!previous || !next) return last.ele
  const span = next.d - previous.d
  const ratio = span > 0 ? (targetM - previous.d) / span : 0
  return previous.ele + (next.ele - previous.ele) * ratio
}

/** Somme des dénivelées positives du profil entre deux distances (m). */
function positiveElevationGain(
  profile: AltimetrieProfilePoint[],
  startM: number,
  endM: number,
): number {
  if (endM <= startM) return 0

  let total = 0
  let previous = elevationAt(profile, startM)
  let index = lowerBound(profile, startM)

  while (index < profile.length && profile[index].d < endM) {
    const elevation = profile[index].ele
    if (elevation > previous) total += elevation - previous
    previous = elevation
    index++
  }

  const final = elevationAt(profile, endM)
  if (final > previous) total += final - previous
  return total
}

/** Premier point du profil dont la distance est ≥ `targetM`. */
function lowerBound(
  profile: AltimetrieProfilePoint[],
  targetM: number,
): number {
  let lo = 0
  let hi = profile.length - 1
  while (lo < hi) {
    const mid = (lo + hi) >> 1
    if (profile[mid].d < targetM) lo = mid + 1
    else hi = mid
  }
  return lo
}

function round(value: number, decimals: number): number {
  const factor = 10 ** decimals
  return Math.round(value * factor) / factor
}

/** Projette une montée persistée vers la forme éditable attendue par le backend. */
export function climbToEdit(climb: AltimetrieClimb): AltimetrieClimbEdit {
  return {
    id: climb.id,
    startKm: climb.startKm,
    endKm: climb.endKm,
    isMerged: climb.isMerged,
    isSeparated: climb.isSeparated,
    groupId: climb.groupId ?? null,
    mergedParts:
      climb.mergedParts?.map(part => ({
        startKm: part.startKm,
        endKm: part.endKm,
      })) ?? null,
    fallback: climb.fallback,
  }
}
