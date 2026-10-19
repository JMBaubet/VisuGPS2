/**
 * Diagnostic de cohérence des paramètres altimétriques.
 *
 * Portage des règles du prototype validé : le lissage d'altitude doit rester
 * nettement plus large que celui de pente, les fenêtres doivent couvrir
 * plusieurs points, et les seuils de fusion/segmentation ne doivent pas être
 * plus fins que le pas d'échantillonnage.
 *
 * Fonction pure, sans dépendance UI.
 */

import type { AltimetrieParams } from '../stores/altimetrie'

export type AltimetrieStatus = 'success' | 'warning' | 'error'

export interface AltimetrieDiagnostics {
  status: AltimetrieStatus
  causes: string[]
}

/** Évalue la cohérence des paramètres et retourne le statut global. */
export function computeAltimetrieDiagnostics(
  params: AltimetrieParams,
): AltimetrieDiagnostics {
  const causes: string[] = []
  let hasError = false
  let hasWarning = false

  const addWarning = (message: string): void => {
    causes.push(message)
    hasWarning = true
  }
  const addError = (message: string): void => {
    causes.push(message)
    hasError = true
  }

  const step = params.pasEchantillonnage
  if (!Number.isFinite(step) || step <= 0) {
    return { status: 'error', causes: ['Pas de rééchantillonnage invalide'] }
  }

  const altitudePoints = Math.round(params.fenetreAltitude / step)
  if (altitudePoints < 5) {
    addError(`Lissage altitude insuffisant (${altitudePoints} points, minimum 5)`)
  } else if (altitudePoints < 10) {
    addWarning(`Lissage altitude court (${altitudePoints} points)`)
  }

  const slopePoints = Math.round(params.fenetrePente / step)
  if (slopePoints < 3) {
    addError(`Lissage de pente insuffisant (${slopePoints} points, minimum 3)`)
  }

  const ratio = params.fenetreAltitude / params.fenetrePente
  if (ratio < 1.5) {
    addError('Lissage altitude trop proche du lissage de pente')
  } else if (ratio < 2) {
    addWarning('Lissage altitude proche du lissage de pente')
  }

  if (
    params.fenetreMediane > 0 &&
    Math.round(params.fenetreMediane / step) < 3
  ) {
    addWarning('Filtre médian trop court')
  }

  if (params.longueurMin > 0 && params.longueurMin < step) {
    addWarning('Longueur minimale des segments inférieure au pas')
  }

  if (params.ecartFusion > 0 && params.ecartFusion < step) {
    addWarning('Écart de fusion inférieur au pas')
  }

  if (params.seuilAntiPic > 0 && params.fenetreAntiPic < 2 * step) {
    addWarning('Fenêtre anti-pic trop courte')
  }

  if (params.seuilAntiPic > 0 && params.seuilAntiPic < 5) {
    addWarning('Seuil anti-pic très bas')
  }

  return {
    status: hasError ? 'error' : hasWarning ? 'warning' : 'success',
    causes,
  }
}
