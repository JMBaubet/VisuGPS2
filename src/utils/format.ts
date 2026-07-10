/**
 * Helpers de formatage pour l'affichage des données de traces.
 *
 * Ces fonctions centralisent les conversions d'unités et le formatage
 * pour garantir un affichage homogène dans toute l'application.
 */

/**
 * Formate une distance en mètres vers une chaîne lisible en kilomètres.
 * Exemple : 123456 → "123.5 km"
 */
export function formatDistance(meters: number): string {
  return `${(meters / 1000).toFixed(1)} km`
}

/**
 * Formate un dénivelé en mètres vers une chaîne lisible.
 * Exemple : 2094 → "2 094 m"
 */
export function formatElevation(meters: number): string {
  const rounded = Math.round(meters)
  // Ajout d'un séparateur de milliers (espace insécable)
  return `${rounded.toLocaleString('fr-FR')} m`
}

/**
 * Formate une durée en secondes vers une chaîne lisible.
 * Exemple : 3661 → "1 h 01 min"
 */
export function formatDuration(seconds: number | null): string {
  if (seconds === null || seconds === undefined) return '—'

  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)

  if (h === 0) return `${m} min`
  if (m === 0) return `${h} h`
  return `${h} h ${String(m).padStart(2, '0')} min`
}

/**
 * Formate une coordonnée géographique en degrés décimaux.
 * Exemple : 2.3491 → "2.3491°"
 */
export function formatCoordinate(value: number): string {
  return `${value.toFixed(4)}°`
}

/**
 * Formate une altitude en mètres.
 * Exemple : 1250.7 → "1 251 m"
 */
export function formatAltitude(meters: number | null): string {
  if (meters === null || meters === undefined) return '—'
  return `${Math.round(meters).toLocaleString('fr-FR')} m`
}
