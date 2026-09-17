// Libellés de la restitution des passages multiples.
//
// La représentation d'un segment se lit sur son emprunt de **référence** : sa
// longueur et son début. Les autres emprunts n'en sont que les répétitions, et
// n'affichent donc que leur propre début.
//
// Fonctions pures, sans dépendance UI : la liste des segments et la fenêtre
// d'action affichent les mêmes libellés, ils sont donc écrits une seule fois.
// Les valeurs sortent en français (`fr-FR`) — virgule décimale et séparateur de
// milliers — comme le reste de l'application.

import type { MultiridePassage } from '../../stores/multiride'

/** Une position ou une longueur en kilomètres : « 11,07 km ». */
export function formatKm(km: number): string {
  return `${km.toLocaleString('fr-FR', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })} km`
}

/** Le début d'un emprunt : « début 51,37 km ». */
export function formatStart(km: number): string {
  return `début ${formatKm(km)}`
}

/**
 * En-tête d'un segment : « Segment 2 : 11,07 km — début 7,62 km ».
 *
 * La longueur et le début sont ceux de l'emprunt de référence. Sans référence
 * — état inexploitable, aucun emprunt de référence dans le segment —, seul le
 * numéro est rendu, plutôt qu'un libellé à trous.
 */
export function formatSegmentTitle(
  segment: number,
  reference: MultiridePassage | null,
): string {
  if (!reference) return `Segment ${segment}`
  return `Segment ${segment} : ${formatKm(reference.longueurKm)} — ${formatStart(reference.kmEntree)}`
}
