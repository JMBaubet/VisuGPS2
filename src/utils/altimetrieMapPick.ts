/**
 * Sélection d'un passage de trace à proximité d'un clic sur la carte.
 *
 * Le clic est exprimé en pixels écran. Les segments de trace sont regroupés
 * en passages **contigus** proches du clic : un tracé qui longe le clic ne
 * produit qu'un candidat, alors qu'une trace multi-trajet qui repasse au même
 * endroit produit un candidat par passage. Le kilomètre de chaque candidat est
 * interpolé le long du segment retenu.
 */

export interface TracePickCandidate {
  /** Distance depuis le départ, en kilomètres. */
  km: number
  /** Distance écran entre le clic et le segment, en pixels. */
  screenDistance: number
  /** Coordonnée interpolée sur la trace, `[lon, lat]`. */
  coordinate: [number, number]
}

/**
 * Retourne un candidat par passage contigu passant à moins de `radiusPx` du
 * clic. Les candidats sont triés par kilomètre croissant.
 */
export function pickTraceCandidates(
  coordinates: Array<[number, number]>,
  screenCoordinates: Array<[number, number]>,
  cumulativeM: number[],
  click: { x: number; y: number },
  radiusPx: number,
): TracePickCandidate[] {
  const count = Math.min(
    coordinates.length,
    screenCoordinates.length,
    cumulativeM.length,
  )
  if (count < 2 || radiusPx <= 0) return []

  const picks: TracePickCandidate[] = []
  let current: TracePickCandidate | null = null

  for (let index = 0; index < count - 1; index++) {
    const from = screenCoordinates[index]
    const to = screenCoordinates[index + 1]
    const dx = to[0] - from[0]
    const dy = to[1] - from[1]
    const lengthSquared = dx * dx + dy * dy

    // Segment dégénéré à l'écran (zoom extrême) : il ne coupe pas le passage.
    if (lengthSquared < 1e-9) continue

    const ratio = clamp(
      ((click.x - from[0]) * dx + (click.y - from[1]) * dy) / lengthSquared,
      0,
      1,
    )
    const projectedX = from[0] + ratio * dx
    const projectedY = from[1] + ratio * dy
    const screenDistance = Math.hypot(
      click.x - projectedX,
      click.y - projectedY,
    )

    if (screenDistance > radiusPx) {
      if (current) {
        picks.push(current)
        current = null
      }
      continue
    }

    const candidate: TracePickCandidate = {
      km:
        (cumulativeM[index] +
          ratio * (cumulativeM[index + 1] - cumulativeM[index])) /
        1000,
      screenDistance,
      coordinate: [
        coordinates[index][0] +
          ratio * (coordinates[index + 1][0] - coordinates[index][0]),
        coordinates[index][1] +
          ratio * (coordinates[index + 1][1] - coordinates[index][1]),
      ],
    }

    if (!current || candidate.screenDistance < current.screenDistance) {
      current = candidate
    }
  }

  if (current) picks.push(current)
  picks.sort((left, right) => left.km - right.km)
  return picks
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max)
}
