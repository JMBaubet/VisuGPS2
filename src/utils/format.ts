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

/**
 * Formate un horodatage ISO 8601 en date et heure locales françaises.
 * Exemple : "2026-10-20T14:32:00Z" → "20/10/2026 14:32"
 *
 * Une valeur absente ou illisible est rendue telle quelle (`—` pour `null`) :
 * l'appelant affiche l'information sans interrompre le rendu.
 */
export function formatDateTime(iso: string | null): string {
  if (!iso) return '—'

  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso

  return date.toLocaleDateString('fr-FR', {
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}

/**
 * Formate une taille en octets vers une chaîne lisible.
 * Exemple : 89128960 → "85 Mo", 1288490188 → "1,20 Go"
 */
export function formatFileSize(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined) return '—'
  if (bytes < 1024) return `${bytes} o`

  const ko = bytes / 1024
  if (ko < 1024) return `${Math.round(ko)} Ko`

  const mo = ko / 1024
  if (mo < 1024) return `${Math.round(mo).toLocaleString('fr-FR')} Mo`

  return `${(mo / 1024).toFixed(2).replace('.', ',')} Go`
}
