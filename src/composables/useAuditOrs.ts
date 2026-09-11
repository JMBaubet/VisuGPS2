/**
 * Échanges avec OpenRouteService (IHM §7.2 et §18).
 *
 * Encapsule le **contrat réseau** : deux profils interrogés en parallèle, un
 * délai maximal de 12 s, l'annulation par l'utilisateur, et la bascule
 * automatique sur la clé de secours en cas de refus de quota (403/429).
 *
 * Les clés proviennent du système de paramètres, chiffrées (`Audit.OpenRouteService.*`,
 * décision 5) — là où le HTML de référence les stockait en clair dans
 * `localStorage`.
 *
 * Le test d'identité des deux tracés est métrique : il est délégué à Rust
 * (`audit_routes_identical`).
 */

import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useSettingsStore } from '../stores/settings'
import { useUiStore } from '../stores/ui'
import type { LatLon, OrsRoute } from '../stores/audit'

/** Délai maximal d'une requête (`ORS_TIMEOUT_MS`). */
export const ORS_TIMEOUT_MS = 12000

const ORS_BASE = 'https://api.openrouteservice.org/v2/directions'

/** Profils interrogés, dans l'ordre du HTML de référence. */
export type OrsProfile = 'driving-car' | 'cycling-road'

/** Résultat d'une demande : les tracés disponibles et leur identité. */
export interface OrsResult {
  car: OrsRoute | null
  bike: OrsRoute | null
  identical: boolean
}

/** Erreur HTTP portant le code de statut (pour la bascule 403/429). */
interface OrsHttpError extends Error {
  status?: number
}

/** Clés disponibles, telles que saisies dans les paramètres. */
export interface OrsKeys {
  primary: string
  secondary: string
}

export function useAuditOrs() {
  const settingsStore = useSettingsStore()
  const ui = useUiStore()

  /** Vraie pendant qu'une demande est en vol (le bouton sert d'annulation). */
  const isLoading = ref(false)
  /**
   * Clé active : n°1 par défaut, n°2 après une bascule de quota.
   * État de session (la référence le persistait ; les clés, elles, sont
   * persistées et chiffrées par le système de paramètres).
   */
  const activeKey = ref<'primary' | 'secondary'>('primary')

  let controller: AbortController | null = null
  let stoppedByUser = false

  /** Lit les deux clés ORS (déchiffrées) depuis les paramètres. */
  async function readKeys(): Promise<OrsKeys> {
    const read = async (path: string): Promise<string> => {
      try {
        const value = await settingsStore.getSettingValue(path)
        return typeof value === 'string' ? value.trim() : ''
      } catch {
        return ''
      }
    }
    return {
      primary: await read('Audit.OpenRouteService.clePrimaire'),
      secondary: await read('Audit.OpenRouteService.cleSecondaire'),
    }
  }

  /** Clé active d'après l'état courant. */
  async function currentKey(): Promise<string> {
    const keys = await readKeys()
    return activeKey.value === 'primary' ? keys.primary : keys.secondary
  }

  /** Clé de secours (celle qui n'est pas active), ou chaîne vide. */
  async function otherKey(): Promise<string> {
    const keys = await readKeys()
    return activeKey.value === 'primary' ? keys.secondary : keys.primary
  }

  /** Bascule sur l'autre clé et retourne son rang (1 ou 2). */
  function switchKey(): number {
    activeKey.value = activeKey.value === 'primary' ? 'secondary' : 'primary'
    return activeKey.value === 'secondary' ? 2 : 1
  }

  /**
   * Interroge un profil ORS.
   *
   * Le contrat d'appel reçoit des objets `{ lat, lon }` et sérialise lui-même
   * au format ORS `[lon, lat]` — c'est l'inversion la plus fréquente lors d'une
   * réimplémentation (§18.2).
   */
  async function fetchRoute(
    profile: OrsProfile,
    a: LatLon,
    b: LatLon,
    key: string,
    signal: AbortSignal,
  ): Promise<OrsRoute> {
    const res = await fetch(`${ORS_BASE}/${profile}/geojson`, {
      method: 'POST',
      headers: {
        Authorization: key,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        coordinates: [
          [a.lon, a.lat],
          [b.lon, b.lat],
        ],
        elevation: true,
      }),
      signal,
    })

    if (!res.ok) {
      let message = `HTTP ${res.status}`
      try {
        const body = await res.json()
        message = body?.error?.message ?? message
      } catch {
        /* corps illisible : le code HTTP fait foi */
      }
      const err: OrsHttpError = new Error(message)
      err.status = res.status
      throw err
    }

    const body = await res.json()
    const feature = body?.features?.[0]
    const raw = feature?.geometry?.coordinates
    if (!Array.isArray(raw) || raw.length === 0) {
      throw new Error('aucun itinéraire')
    }

    const coords: LatLon[] = raw.map((c: number[]) => ({
      lat: c[1],
      lon: c[0],
    }))
    const summary = feature?.properties?.summary ?? {}
    return {
      coords,
      distance: summary.distance ?? 0,
      duration: summary.duration ?? 0,
    }
  }

  /** Cause d'un rejet, telle qu'affichée à l'utilisateur (§18.4). */
  function reasonOf(result: PromiseSettledResult<OrsRoute>): string | null {
    if (result.status !== 'rejected') return null
    const reason = result.reason as OrsHttpError | undefined
    if (reason?.name === 'AbortError') {
      return `délai dépassé (${Math.round(ORS_TIMEOUT_MS / 1000)} s)`
    }
    return reason?.message ?? 'service injoignable'
  }

  /**
   * Demande les deux tracés (voiture et vélo de route).
   *
   * Retourne `null` si la demande a échoué ou a été annulée. Si un appel est
   * déjà en vol, un nouvel appel **annule** le précédent (le bouton sert de
   * bascule « demander / annuler »).
   */
  async function requestRoutes(a: LatLon, b: LatLon): Promise<OrsResult | null> {
    // Verrou : une demande en cours → annulation prioritaire.
    if (controller) {
      stoppedByUser = true
      controller.abort()
      return null
    }

    // Aucune clé active mais une clé de secours : bascule d'office.
    if (!(await currentKey())) {
      if (await otherKey()) {
        switchKey()
      } else {
        ui.showError('Renseignez au moins une clé API OpenRouteService.')
        return null
      }
    }

    isLoading.value = true
    controller = new AbortController()
    stoppedByUser = false
    let timer = window.setTimeout(() => controller?.abort(), ORS_TIMEOUT_MS)

    try {
      let car: OrsRoute | null = null
      let bike: OrsRoute | null = null

      for (let attempt = 1; ; attempt++) {
        const key = await currentKey()
        const signal = controller.signal
        const [carRes, bikeRes] = await Promise.allSettled([
          fetchRoute('driving-car', a, b, key, signal),
          fetchRoute('cycling-road', a, b, key, signal),
        ])
        window.clearTimeout(timer)

        // L'annulation utilisateur prime : ni bascule, ni relance.
        if (stoppedByUser) {
          ui.showInfo('Requête annulée — vous pouvez relancer immédiatement.')
          return null
        }

        car = carRes.status === 'fulfilled' ? carRes.value : null
        bike = bikeRes.status === 'fulfilled' ? bikeRes.value : null
        if (car || bike) break

        const quotaFail = [carRes, bikeRes].some(
          (r) =>
            r.status === 'rejected' &&
            (r.reason?.status === 403 || r.reason?.status === 429),
        )

        // Bascule : réservée à la 1ʳᵉ tentative, à un échec total, et à la
        // présence d'une clé de secours (§18.4).
        if (attempt === 1 && quotaFail && (await otherKey())) {
          const previous = activeKey.value === 'primary' ? 1 : 2
          const next = switchKey()
          ui.showInfo(
            `Clé ${previous} refusée (quota ?) — nouvelle tentative avec la clé ${next}.`,
          )
          timer = window.setTimeout(() => controller?.abort(), ORS_TIMEOUT_MS)
          continue
        }

        if (attempt === 2 && quotaFail) {
          ui.showError(
            'Routage impossible : les deux clés sont refusées (403) — quota épuisé ou clés invalides.',
          )
        } else {
          const why = [reasonOf(carRes), reasonOf(bikeRes)]
            .filter(Boolean)
            .join(' · ')
          ui.showError(
            `Routage impossible : ${why || 'service injoignable.'} — relancez, ou réduisez la zone avec les curseurs.`,
          )
        }
        return null
      }

      const identical = await invoke<boolean>('audit_routes_identical', {
        car: car?.coords ?? [],
        carDistance: car?.distance ?? 0,
        bike: bike?.coords ?? [],
        bikeDistance: bike?.distance ?? 0,
      })

      return { car, bike, identical }
    } catch (error) {
      const reason = error as OrsHttpError | undefined
      ui.showError(`Routage impossible : ${reason?.message ?? 'erreur inattendue.'}`)
      return null
    } finally {
      window.clearTimeout(timer)
      controller = null
      isLoading.value = false
    }
  }

  /** Annule la demande en cours, s'il y en a une. */
  function cancel(): void {
    if (!controller) return
    stoppedByUser = true
    controller.abort()
  }

  return { isLoading, activeKey, requestRoutes, cancel, readKeys }
}
