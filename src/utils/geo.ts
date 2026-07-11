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
