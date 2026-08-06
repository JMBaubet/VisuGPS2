/**
 * Utilitaires géographiques pour le calcul de distances.
 *
 * Fonctions pures, nommées, exportées — même pattern que format.ts.
 * Le rayon terrestre (R = 6 371 000 m) est cohérent avec la fonction
 * haversine() du backend Rust (import_gpx.rs).
 */

/** Convertit des degrés décimaux en radians. */
export function toRadians(deg: number): number {
  return (deg * Math.PI) / 180
}

/**
 * Distance géodésique entre deux points (formule de Haversine), en mètres.
 *
 * @param lat1 - Latitude du premier point (degrés décimaux).
 * @param lon1 - Longitude du premier point (degrés décimaux).
 * @param lat2 - Latitude du deuxième point (degrés décimaux).
 * @param lon2 - Longitude du deuxième point (degrés décimaux).
 * @returns Distance en mètres.
 */
export function haversineMeters(
  lat1: number,
  lon1: number,
  lat2: number,
  lon2: number,
): number {
  const R = 6_371_000 // rayon terrestre moyen en mètres
  const dLat = toRadians(lat2 - lat1)
  const dLon = toRadians(lon2 - lon1)

  const a =
    Math.sin(dLat / 2) ** 2 +
    Math.cos(toRadians(lat1)) * Math.cos(toRadians(lat2)) * Math.sin(dLon / 2) ** 2

  return R * 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a))
}

/**
 * Cap géographique initial (bearing) entre deux points, en degrés [0, 360).
 *
 * Le cap est mesuré depuis le nord, dans le sens horaire. Utilisé pour
 * orienter la caméra (keyframes) et calculer la relation angulaire entre
 * la direction de la caméra et le marqueur (HUD télémétrie).
 *
 * @param lat1 - Latitude du point de départ (degrés décimaux).
 * @param lon1 - Longitude du point de départ (degrés décimaux).
 * @param lat2 - Latitude du point d'arrivée (degrés décimaux).
 * @param lon2 - Longitude du point d'arrivée (degrés décimaux).
 * @returns Cap initial en degrés, normalisé sur [0, 360).
 */
export function bearing(
  lat1: number,
  lon1: number,
  lat2: number,
  lon2: number,
): number {
  const phi1 = toRadians(lat1)
  const phi2 = toRadians(lat2)
  const dLambda = toRadians(lon2 - lon1)

  const y = Math.sin(dLambda) * Math.cos(phi2)
  const x =
    Math.cos(phi1) * Math.sin(phi2) -
    Math.sin(phi1) * Math.cos(phi2) * Math.cos(dLambda)

  const theta = Math.atan2(y, x)
  const deg = (theta * 180) / Math.PI
  return (deg + 360) % 360
}

/**
 * Différence angulaire entre deux caps, normalisée sur [-180, 180].
 *
 * Permet d'exprimer l'écart relatif entre la direction d'une caméra et le
 * cap vers le marqueur (ex. +30° = marqueur à droite, -30° = à gauche).
 *
 * @param from - Cap de référence (degrés), ex. direction caméra.
 * @param to   - Cap cible (degrés), ex. cap vers le marqueur.
 * @returns Écart angulaire en degrés sur [-180, 180].
 */
export function bearingDelta(from: number, to: number): number {
  const diff = ((to - from + 540) % 360) - 180
  return diff
}
