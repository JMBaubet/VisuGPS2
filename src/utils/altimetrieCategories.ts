/**
 * Catégories de pente de l'altimétrie.
 *
 * Palette et bornes reprises du prototype validé (`features/altimetrie/`) :
 * bornes hautes exclusives, `±Infinity` pour les extrémités. Ces catégories
 * servent au trait horizontal des montées, au graphe et à la carte.
 *
 * Fonctions pures, sans dépendance UI.
 */

export interface AltimetrieCategory {
  id: string
  label: string
  color: string
  /** Borne basse inclusive (peut être -Infinity). */
  min: number
  /** Borne haute exclusive (peut être Infinity). */
  max: number
}

export const ALTIMETRIE_CATEGORIES: readonly AltimetrieCategory[] = [
  { id: 'dh3', label: 'Descente raide', color: '#1e40af', min: -Infinity, max: -7 },
  { id: 'dh2', label: 'Descente', color: '#3b82f6', min: -7, max: -3 },
  { id: 'dh1', label: 'Faux plat descendant', color: '#93c5fd', min: -3, max: -1 },
  { id: 'pl', label: 'Plat', color: '#22c55e', min: -1, max: 1 },
  { id: 'up1', label: 'Faux plat montant', color: '#fde68a', min: 1, max: 3 },
  { id: 'up2', label: 'Montée', color: '#f97316', min: 3, max: 7 },
  { id: 'up3', label: 'Montée raide', color: '#dc2626', min: 7, max: 10 },
  { id: 'up4', label: 'Montée très raide', color: '#000000', min: 10, max: Infinity },
]

/** Catégorie de repli (plat), utilisée pour une pente NaN. */
const PLATEAU = ALTIMETRIE_CATEGORIES[3]!

/** Retourne la catégorie d'une pente exprimée en pourcentage. */
export function categorizeSlope(slope: number): AltimetrieCategory {
  if (Number.isNaN(slope)) return PLATEAU
  return (
    ALTIMETRIE_CATEGORIES.find(
      category => slope >= category.min && slope < category.max,
    ) ?? PLATEAU
  )
}

/** Couleur d'une pente (catégorie de pente). */
export function colorForSlope(slope: number): string {
  return categorizeSlope(slope).color
}

/** Libellé d'une pente (catégorie de pente). */
export function labelForSlope(slope: number): string {
  return categorizeSlope(slope).label
}

/** Couleur d'une catégorie par son identifiant (`dh3`…`up4`). */
export function colorForCategoryId(id: string): string {
  return (
    ALTIMETRIE_CATEGORIES.find(category => category.id === id)?.color ?? '#64748b'
  )
}
