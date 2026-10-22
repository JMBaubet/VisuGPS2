/**
 * Utilitaires purs du module Tunnels (aucune dépendance UI).
 *
 * - dérivation des groupes fusionnés (tronçons disjoints partageant un
 *   `groupId`) ;
 * - contraintes de déplacement des bornes, alignées sur celles des montées ;
 * - seuils d'affichage court/long, partagés par la carte de circuit, le widget
 *   de profil et le mini-profil.
 */

import type { Tunnel, TunnelGroup } from '../stores/tunnels'

/** Longueur minimale d'un tunnel manuel (10 m), miroir du backend. */
export const MIN_TUNNEL_LENGTH_KM = 0.01

/** Écart minimal entre deux bornes voisines (1 m). */
export const MIN_TUNNEL_GAP_KM = 0.001

function round6(value: number): number {
  return Math.round(value * 1e6) / 1e6
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max)
}

/** Deux tunnels se chevauchent-ils ? Les bornes jointives sont admises. */
export function tunnelRangeOverlaps(
  tunnels: Tunnel[],
  startKm: number,
  endKm: number,
  ignoreIds: string[] = [],
): boolean {
  const ignored = new Set(ignoreIds)
  return tunnels.some(tunnel => {
    if (ignored.has(tunnel.id)) return false
    return startKm < tunnel.endKm - 1e-9 && endKm > tunnel.startKm + 1e-9
  })
}

/**
 * Dérive les groupes affichables : un groupe fusionné est une entrée unique,
 * ses tronçons restent distincts et disjoints. La longueur d'un groupe est la
 * **somme** des longueurs de ses tronçons, jamais l'étendue qui inclut l'écart.
 */
export function groupTunnels(tunnels: Tunnel[]): TunnelGroup[] {
  const sorted = [...tunnels].sort((a, b) => a.startKm - b.startKm)
  const groups: TunnelGroup[] = []
  const byGroupId = new Map<string, TunnelGroup>()

  for (const tunnel of sorted) {
    const groupId = tunnel.groupId ?? null
    if (groupId) {
      let group = byGroupId.get(groupId)
      if (!group) {
        group = {
          id: tunnel.id,
          groupId,
          tunnels: [],
          startKm: tunnel.startKm,
          endKm: tunnel.endKm,
          lengthM: 0,
          merged: false,
        }
        byGroupId.set(groupId, group)
        groups.push(group)
      }
      group.tunnels.push(tunnel)
      group.startKm = Math.min(group.startKm, tunnel.startKm)
      group.endKm = Math.max(group.endKm, tunnel.endKm)
    } else {
      groups.push({
        id: tunnel.id,
        groupId: null,
        tunnels: [tunnel],
        startKm: tunnel.startKm,
        endKm: tunnel.endKm,
        lengthM: tunnel.lengthM,
        merged: false,
      })
    }
  }

  for (const group of groups) {
    group.tunnels.sort((a, b) => a.startKm - b.startKm)
    group.id = group.tunnels[0].id
    group.merged = group.tunnels.length > 1
    group.lengthM = group.tunnels.reduce(
      (total, tunnel) => total + tunnel.lengthM,
      0,
    )
  }

  groups.sort((a, b) => a.startKm - b.startKm)
  return groups
}

/**
 * Contraint une borne de tunnel entre ses voisins : 1 m d'écart et 10 m de
 * longueur minimale, comme pour les montées.
 */
export function constrainTunnelBound(
  tunnel: Tunnel,
  handle: 'entrance' | 'exit',
  km: number,
  tunnels: Tunnel[],
  totalKm: number,
): { startKm: number; endKm: number } {
  const sorted = [...tunnels].sort((a, b) => a.startKm - b.startKm)
  const index = sorted.findIndex(candidate => candidate.id === tunnel.id)
  if (index < 0) {
    return { startKm: tunnel.startKm, endKm: tunnel.endKm }
  }

  const previous = index > 0 ? sorted[index - 1] : null
  const next = index < sorted.length - 1 ? sorted[index + 1] : null
  const total = Math.max(totalKm, tunnel.endKm)

  if (handle === 'entrance') {
    const min = Math.max(0, (previous?.endKm ?? 0) + MIN_TUNNEL_GAP_KM)
    const max = tunnel.endKm - MIN_TUNNEL_LENGTH_KM
    if (max < min) {
      return { startKm: tunnel.startKm, endKm: tunnel.endKm }
    }
    return {
      startKm: round6(clamp(km, min, max)),
      endKm: tunnel.endKm,
    }
  }

  const min = tunnel.startKm + MIN_TUNNEL_LENGTH_KM
  const max = Math.min(total, (next?.startKm ?? total) - MIN_TUNNEL_GAP_KM)
  if (max < min) {
    return { startKm: tunnel.startKm, endKm: tunnel.endKm }
  }
  return {
    startKm: tunnel.startKm,
    endKm: round6(clamp(km, min, max)),
  }
}

/** Applique des bornes à un tunnel et recalcule sa longueur dérivée. */
export function applyTunnelBounds(
  tunnel: Tunnel,
  startKm: number,
  endKm: number,
): Tunnel {
  return {
    ...tunnel,
    startKm,
    endKm,
    lengthM: Math.max(0, (endKm - startKm) * 1000),
  }
}

/** Longueur formatée : mètres sous 1 km, kilomètres au-delà. */
export function formatTunnelLength(lengthM: number): string {
  if (!Number.isFinite(lengthM)) return '—'
  if (lengthM >= 1000) return `${(lengthM / 1000).toFixed(2)} km`
  return `${Math.round(lengthM)} m`
}

/**
 * Tunnel « court » sur un mini-profil : moins de 2 % de la largeur totale.
 * Il est alors dessiné comme un trait vertical pointillé plutôt qu'une bande.
 */
export function isShortTunnelOnProfile(
  startKm: number,
  endKm: number,
  totalDistanceM: number,
  ratio = 0.02,
): boolean {
  if (totalDistanceM <= 0) return true
  return ((endKm - startKm) * 1000) / totalDistanceM < ratio
}

/** Tunnel « court » dans le widget : moins de 8 px à l'échelle X courante. */
export function isShortTunnelOnWidget(
  startKm: number,
  endKm: number,
  xScaleM: number,
  thresholdPx = 8,
): boolean {
  if (xScaleM <= 0) return true
  return ((endKm - startKm) * 1000) / xScaleM < thresholdPx
}
