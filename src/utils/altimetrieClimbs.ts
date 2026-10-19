/**
 * Helpers purs de manipulation des montées.
 *
 * - `constrainClimbBound` applique les contraintes de déplacement du prototype :
 *   pas de chevauchement (1 m entre montées) et longueur minimale de 50 m.
 * - `climbToEdit` projette l'état persisté vers la forme envoyée à
 *   `altimetrie_preview` / `altimetrie_save` ; le backend recalcule toutes les
 *   métriques et ne fait jamais confiance aux valeurs dérivées.
 */

import type { AltimetrieClimb, AltimetrieClimbEdit } from '../stores/altimetrie'

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
