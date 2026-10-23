/**
 * Helpers purs de la gestion des données OSM.
 *
 * Aucune dépendance Vue : l'icône de la toolbar et la fenêtre de gestion leur
 * passent l'état du store et reçoivent un affichage prêt à rendre.
 */

import type { OsmFileStatus, OsmSupportOverview } from '../stores/osm'
import { formatFileSize } from './format'

export interface OsmStatusDisplay {
  icon: 'mdi-usb' | 'mdi-map-legend'
  color: string
  tooltip: string[]
  action: 'settings' | 'dialog'
}

function stateColor(state: string): string {
  switch (state) {
    case 'green':
      return 'green'
    case 'yellow':
      return 'yellow-darken-2'
    case 'orange':
      return 'orange-darken-2'
    default:
      return 'grey'
  }
}

/**
 * Décrit l'icône de la toolbar : `mdi-usb` rouge si le support manque, sinon
 * `mdi-map-legend` coloré selon le fichier le plus critique.
 */
export function describeOsmOverview(
  overview: OsmSupportOverview | null,
  loading: boolean,
  error: string | null,
): OsmStatusDisplay {
  if (error && !overview) {
    return {
      icon: 'mdi-usb',
      color: 'red',
      tooltip: ['État des données OSM indisponible.', error],
      action: 'dialog',
    }
  }

  if (!overview) {
    return {
      icon: 'mdi-map-legend',
      color: 'grey',
      tooltip: [
        loading
          ? "Chargement de l'état des données OSM…"
          : "État des données OSM inconnu.",
      ],
      action: 'dialog',
    }
  }

  switch (overview.state) {
    case 'not_configured':
      return {
        icon: 'mdi-usb',
        color: 'red',
        tooltip: [
          "Aucun dossier OSM n'est configuré.",
          'Cliquez pour ouvrir les paramètres.',
        ],
        action: 'settings',
      }
    case 'usb_missing':
      return {
        icon: 'mdi-usb',
        color: 'red',
        tooltip: [
          `Support OSM introuvable : ${overview.folder ?? 'dossier inconnu'}.`,
          'Vérifiez que la clé USB est branchée.',
          'Cliquez pour gérer les données OSM.',
        ],
        action: 'dialog',
      }
    case 'empty':
      return {
        icon: 'mdi-map-legend',
        color: 'grey',
        tooltip: [
          'Aucun fichier PBF présent.',
          'Cliquez pour télécharger une région.',
        ],
        action: 'dialog',
      }
    default: {
      const tooltip = [
        `${overview.files.length} fichier${
          overview.files.length > 1 ? 's' : ''
        } · ${formatFileSize(overview.totalSizeBytes)}`,
        `Espace libre : ${formatFileSize(overview.freeBytes)}`,
      ]
      const toRenew = overview.files.filter(
        file => file.state === 'yellow',
      ).length
      const endOfLife = overview.files.filter(
        file => file.state === 'orange',
      ).length
      if (toRenew > 0) tooltip.push(`${toRenew} à renouveler`)
      if (endOfLife > 0) tooltip.push(`${endOfLife} en fin de vie`)
      tooltip.push('Cliquez pour gérer les données OSM.')
      return {
        icon: 'mdi-map-legend',
        color: stateColor(overview.state),
        tooltip,
        action: 'dialog',
      }
    }
  }
}

export function fileStateColor(state: string): string {
  return stateColor(state)
}

export function fileStateLabel(state: string): string {
  switch (state) {
    case 'green':
      return 'À jour'
    case 'yellow':
      return 'À renouveler'
    case 'orange':
      return 'Fin de vie'
    default:
      return 'Inconnu'
  }
}

/** Libellé des jours restants ou dépassés par rapport au renouvellement. */
export function daysLabel(file: OsmFileStatus): string {
  if (file.dateSource === 'unknown') return 'Date inconnue'
  if (file.daysRemaining > 0) return `${file.daysRemaining} j restants`
  if (file.daysRemaining === 0) return "À renouveler aujourd'hui"
  return `${Math.abs(file.daysRemaining)} j de retard`
}
