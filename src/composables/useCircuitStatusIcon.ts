/**
 * Icône combinée `mdi-tunnel` / `mdi-terrain` du composant Circuit.vue.
 *
 * Fonction pure, sans dépendance Vue : le composant construit les deux états
 * (altitude et tunnels) et reçoit l'affichage à rendre. L'ordre de priorité est
 * celui du §2.3 de la Note technique :
 *
 * 1. altitude verrouillée (trace non nettoyée) ;
 * 2. détection des tunnels en cours ;
 * 3. erreur de détection des tunnels ;
 * 4. tunnels en attente de validation ;
 * 5. altimétrie en attente de validation ;
 * 6. support OSM absent et aucun fichier de tunnels ;
 * 7. altimétrie validée avec montées ;
 * 8. altimétrie validée sans montée.
 */

export type AltitudeState = 'locked' | 'pending' | 'validated'

export type TunnelStatusState =
  | 'hidden'
  | 'disabled'
  | 'pending'
  | 'error'
  | 'waiting_validate'
  | 'success'
  | 'not_configured'
  | 'missing_pbf'

export type CircuitStatusAction =
  | 'none'
  | 'warning-altitude'
  | 'altimetrie-climbs'
  | 'altimetrie-tunnels'
  | 'relaunch-tunnels'
  | 'settings'

export interface CircuitAltitudeStatus {
  state: AltitudeState
  climbCount: number
}

export interface CircuitTunnelStatus {
  state: TunnelStatusState
  /** `tunnels.json` existe et porte un statut waiting_validate/success. */
  hasFile: boolean
  count?: number | null
  message?: string | null
}

export interface CircuitStatusDisplay {
  icon: 'mdi-tunnel' | 'mdi-terrain'
  color: string
  visibility: 'hover' | 'always'
  tooltip: string
  action: CircuitStatusAction
  loading: boolean
}

export function computeCircuitStatusDisplay(
  altitude: CircuitAltitudeStatus,
  tunnel: CircuitTunnelStatus | null,
): CircuitStatusDisplay {
  if (altitude.state === 'locked') {
    return {
      icon: 'mdi-terrain',
      color: 'grey',
      visibility: 'hover',
      tooltip: "L'altimétrie n'est disponible qu'après le nettoyage de la trace.",
      action: 'warning-altitude',
      loading: false,
    }
  }

  const candidate =
    tunnel && tunnel.state !== 'hidden' && tunnel.state !== 'disabled'
      ? tunnel
      : null

  if (candidate?.state === 'pending') {
    return {
      icon: 'mdi-tunnel',
      color: 'orange',
      visibility: 'always',
      tooltip: 'Détection des tunnels en cours…',
      action: 'altimetrie-tunnels',
      loading: true,
    }
  }

  if (candidate?.state === 'error') {
    const detail = candidate.message ? ` : ${candidate.message}` : ''
    return {
      icon: 'mdi-tunnel',
      color: 'red',
      visibility: 'always',
      tooltip: `Erreur de détection des tunnels${detail}. Cliquez pour relancer.`,
      action: 'relaunch-tunnels',
      loading: false,
    }
  }

  if (candidate?.state === 'waiting_validate') {
    const count = candidate.count ?? 0
    const label =
      count > 0
        ? `${count} tunnel${count > 1 ? 's' : ''} à valider. Cliquez pour ouvrir l'Altimétrie.`
        : "Tunnels à valider. Cliquez pour ouvrir l'Altimétrie."
    return {
      icon: 'mdi-tunnel',
      color: 'yellow-darken-2',
      visibility: 'always',
      tooltip: label,
      action: 'altimetrie-tunnels',
      loading: false,
    }
  }

  if (altitude.state === 'pending') {
    return {
      icon: 'mdi-terrain',
      color: 'yellow-darken-2',
      visibility: 'always',
      tooltip: "Analyser l'altimétrie — validation requise.",
      action: 'altimetrie-climbs',
      loading: false,
    }
  }

  if (
    candidate &&
    !candidate.hasFile &&
    (candidate.state === 'not_configured' || candidate.state === 'missing_pbf')
  ) {
    if (candidate.state === 'missing_pbf') {
      return {
        icon: 'mdi-tunnel',
        color: 'grey',
        visibility: 'hover',
        tooltip:
          "Données OSM manquantes ou en cours de téléchargement. Cliquez pour ouvrir l'onglet Tunnels.",
        action: 'altimetrie-tunnels',
        loading: false,
      }
    }
    return {
      icon: 'mdi-tunnel',
      color: 'grey',
      visibility: 'hover',
      tooltip:
        "Support OSM non configuré. Cliquez pour ouvrir l'onglet Tunnels et ses paramètres.",
      action: 'altimetrie-tunnels',
      loading: false,
    }
  }

  if (altitude.state === 'validated') {
    if (altitude.climbCount > 0) {
      return {
        icon: 'mdi-terrain',
        color: 'green',
        visibility: 'hover',
        tooltip: 'Altimétrie validée — consulter l’analyse.',
        action: 'altimetrie-climbs',
        loading: false,
      }
    }
    return {
      icon: 'mdi-terrain',
      color: 'white',
      visibility: 'hover',
      tooltip: 'Altimétrie validée — aucune montée détectée.',
      action: 'altimetrie-climbs',
      loading: false,
    }
  }

  return {
    icon: 'mdi-terrain',
    color: 'grey',
    visibility: 'hover',
    tooltip: 'État inconnu.',
    action: 'none',
    loading: false,
  }
}
