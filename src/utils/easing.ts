/**
 * Fonctions d'easing pour le lissage des transitions (overrides de montage).
 *
 * Une fonction d'easing mappe un `t` ∈ [0, 1] (progression normalisée de la
 * transition) vers un facteur d'application ∈ [0, 1] (0 = valeur brute,
 * 1 = valeur override). Au cœur de la plage, le facteur vaut 1 (override
 * pleinement appliqué) ; aux bords, il ramène progressivement à la valeur
 * brute pour éviter les cassures.
 *
 * Fonctions pures, exportées nommément — même pattern que geo.ts / format.ts.
 */

import type { EasingType } from './keyframes'

/** Interpolation linéaire : transition uniforme (aucun lissage). */
export function linear(t: number): number {
  return t
}

/**
 * Smoothstep : accélération puis décélération douces (polynôme cubique).
 * Recommandé pour des transitions caméra naturelles.
 */
export function smoothstep(t: number): number {
  const x = clamp01(t)
  return x * x * (3 - 2 * x)
}

/**
 * easeInOut : démarrage et fin lents, accélération au milieu (sinusoïde).
 * Plus marqué que smoothstep.
 */
export function easeInOut(t: number): number {
  const x = clamp01(t)
  return -(Math.cos(Math.PI * x) - 1) / 2
}

/** Borner une valeur dans [0, 1]. */
function clamp01(t: number): number {
  return t < 0 ? 0 : t > 1 ? 1 : t
}

/**
 * Sélectionne une fonction d'easing par son nom.
 * `linear` par défaut si le nom est inconnu (robustesse).
 */
export function easingFn(name: EasingType | string): (t: number) => number {
  switch (name) {
    case 'smoothstep': return smoothstep
    case 'easeInOut': return easeInOut
    case 'linear':
    default: return linear
  }
}

// --- Interpolations géométriques ---

/** Interpolation linéaire (LERP) entre deux valeurs. */
export function lerp(a: number, b: number, t: number): number {
  return a + (b - a) * t
}

/**
 * Interpolation linéaire sphérique (SLERP) pour les angles (bearing).
 *
 * Le bearing étant cyclique (0° = 360°), une interpolation naïve de 350° à 10°
 * ferait presque un tour complet. Le SLERP prend le chemin le plus court sur
 * le cercle (cf. glossaire §13 du document de spécification).
 *
 * @param a - Angle de départ (degrés).
 * @param b - Angle d'arrivée (degrés).
 * @param t - Paramètre d'interpolation ∈ [0, 1].
 * @returns Angle interpolé (degrés), normalisé dans [0, 360[.
 */
export function slerpAngle(a: number, b: number, t: number): number {
  // Différence la plus courte signée dans [-180, 180].
  let diff = ((b - a) % 360 + 540) % 360 - 180
  let result = a + diff * t
  // Normaliser dans [0, 360[.
  result = ((result % 360) + 360) % 360
  return result
}
