/**
 * Helpers purs du client `hermes` (aucune dépendance UI).
 *
 * - décodage des erreurs préfixées par un code machine (`HERMES_OFFLINE: …`) ;
 * - description de l'état affiché par le chip de la toolbar ;
 * - validation et mesure des polygones custom (dessin Mapbox) ;
 * - limites du serveur pour les avertissements souples de l'IHM.
 *
 * Les fonctions sont sans état et sans accès réseau : les stores et les
 * composants les consomment, ce qui les rend testables et réutilisables.
 */

import { toRadians } from './geo'

/** Code machine d'une erreur hermes, ou `HERMES_UNKNOWN`. */
export interface HermesErrorInfo {
  code: string
  message: string
}

/**
 * Décode une erreur remontée par une commande Tauri `hermes_*`.
 *
 * Les commandes renvoient `Result<T, String>` et préfixent la chaîne par un
 * code (`HERMES_OFFLINE: Connexion…`). Les erreurs de connexion (chaîne reqwest
 * brute) sont résumées pour l'utilisateur ; une erreur sans code connu est
 * classée `HERMES_UNKNOWN`.
 */
export function parseHermesError(error: unknown): HermesErrorInfo {
  const raw = error instanceof Error ? error.message : String(error)
  const trimmed = raw.trim()
  const match = /^(HERMES_[A-Z_]+):\s*(.+)$/s.exec(trimmed)
  if (match) {
    const code = match[1]
    const message = match[2]
    if (code === 'HERMES_OFFLINE') {
      return {
        code,
        message: 'Serveur hermes injoignable (Tailscale ou veille).',
      }
    }
    if (code === 'HERMES_TIMEOUT') {
      return {
        code,
        message: 'Le serveur hermes ne répond pas (délai dépassé).',
      }
    }
    return { code, message }
  }
  const networkError =
    /tcp connect error|connection refused|error sending request/i.test(trimmed)
  if (networkError) {
    return {
      code: 'HERMES_OFFLINE',
      message: 'Serveur hermes injoignable (Tailscale ou veille).',
    }
  }
  return { code: 'HERMES_UNKNOWN', message: trimmed || 'Erreur hermes inconnue.' }
}

/**
 * État de connexion au **serveur** (machine + hermes-api joignable). Distinct
 * de l'état du **service** (instance ORS / dataset servi).
 */
export type HermesServerState =
  | 'unknown'
  | 'online'
  | 'offline'
  | 'waking'
  | 'lost'

/** Mode de l'outil de dessin de polygones sur la carte Hermes. */
export type HermesDrawMode = 'idle' | 'create' | 'edit'

/** Onglet actif du panneau Hermes ; la carte adapte ses couches. */
export type HermesTab = 'datasets' | 'polygons'

/** Vue minimale d'un artefact aux pour juger de sa disponibilité. */
export interface AuxFileLike {
  exists: boolean
  state?: string | null
}

/**
 * Un artefact aux est prêt si son marqueur v1.10 vaut `ready`. Pour un serveur
 * antérieur qui ne fournit pas `state`, on retombe sur `exists`.
 * En particulier, un dossier `mm/` non vide ne suffit pas (contrat).
 */
export function auxFileReady(file: AuxFileLike): boolean {
  return file.state ? file.state === 'ready' : file.exists
}

/** Vue minimale d'un dataset nécessaire à l'affichage du chip. */
export interface HermesDatasetStatusLike {
  id: string
  /** Nom d'affichage (repli sur `id` s'il est absent du manifest). */
  name?: string | null
  status: string
  /** État v1.9 (initializing, building, ready, degraded*, unavailable). */
  state?: string | null
  stateLabel?: string | null
  /** Artefacts v1.9 : `pbf`, `ors`, `communes`, `tunnels`, `mm`. */
  artifacts?: Record<string, unknown> | null
  stale: boolean
}

/** État d'un artefact v1.9 (`ready`, `pending`, `preparing`, `failed`, …). */
export function datasetArtifactState(
  dataset: HermesDatasetStatusLike,
  key: 'pbf' | 'ors' | 'communes' | 'tunnels' | 'mm',
): string | null {
  const value = dataset.artifacts?.[key]
  if (!value || typeof value !== 'object') return null
  const state = (value as Record<string, unknown>).state
  return typeof state === 'string' ? state : null
}

/**
 * Le graphe ORS est-il construit ? Avec v1.9, `status` peut rester
 * `preparing` pendant la génération aux : c'est l'artefact `ors` qui marque la
 * fin du build. Le champ historique `status=ready` reste accepté.
 */
export function isDatasetGraphReady(
  dataset: HermesDatasetStatusLike,
): boolean {
  if (dataset.status === 'ready' || dataset.state === 'ready') return true
  return datasetArtifactState(dataset, 'ors') === 'ready'
}

/** Affichage prêt à rendre pour le chip de la toolbar. */
export interface HermesStatusDisplay {
  icon: string
  color: string
  label: string
  tooltip: string[]
}

function stateLabel(state: HermesServerState): string {
  switch (state) {
    case 'online':
      return 'en ligne'
    case 'offline':
      return 'hors ligne'
    case 'waking':
      return 'réveil en cours'
    case 'lost':
      return 'serveur perdu'
    default:
      return 'état inconnu'
  }
}

/** Libellé d'état d'un dataset, avec repli sur le champ historique `status`. */
export function datasetStateLabel(dataset: HermesDatasetStatusLike): string {
  return dataset.stateLabel ?? dataset.state ?? dataset.status
}

/** Nom d'affichage d'un dataset : le serveur peut l'omettre (repli `id`). */
export function datasetDisplayName(dataset: {
  id: string
  name?: string | null
}): string {
  const name = dataset.name?.trim()
  return name ? name : dataset.id
}

/** Vue minimale d'un polygone pour résoudre les noms des secteurs assemblés. */
export interface HermesPolygonNameLike {
  id: string
  name: string
}

/** Noms des secteurs assemblés par un dataset (repli sur l'id inconnu). */
export function datasetSectorNames(
  dataset: { zones?: string[] | null } | null | undefined,
  polygons: HermesPolygonNameLike[],
): string[] {
  return (dataset?.zones ?? []).map(
    polygonId =>
      polygons.find(polygon => polygon.id === polygonId)?.name ?? polygonId,
  )
}

/** Vue minimale d'un polygone pour juger de sa révision après un build. */
export interface HermesPolygonRevisionLike {
  id: string
  updatedAt?: string | null
}

/** Vue minimale d'un dataset zones pour juger de sa fraîcheur. */
export interface HermesDatasetUpdateLike extends HermesDatasetStatusLike {
  zones?: string[] | null
  createdAt?: string | null
}

/**
 * Un dataset en mode zones doit-il être reconstruit après une modification de
 * secteur ?
 *
 * Heuristique provisoire, en attendant un état serveur fiable : un polygone
 * modifié après la création du dataset (date de lancement du build) est
 * considéré comme plus récent que le clippage. La liste des polygones doit
 * être chargée, sinon on ne conclut rien.
 */
export function datasetNeedsUpdate(
  dataset: HermesDatasetUpdateLike,
  polygons: HermesPolygonRevisionLike[],
): boolean {
  const zones = dataset.zones ?? []
  if (zones.length === 0 || polygons.length === 0) return false
  const builtAt = Date.parse(dataset.createdAt ?? '')
  if (!Number.isFinite(builtAt)) return false
  return zones.some(zoneId => {
    const polygon = polygons.find(candidate => candidate.id === zoneId)
    // Secteur supprimé depuis le build : le dataset ne peut plus être régénéré
    // à l'identique, il est à mettre à jour.
    if (!polygon) return true
    const updatedAt = Date.parse(polygon.updatedAt ?? '')
    return Number.isFinite(updatedAt) && updatedAt > builtAt
  })
}

/**
 * Artefacts auxiliaires dégradés d'un dataset, dans l'ordre d'affichage.
 *
 * Les états v1.9 `degraded*` nomment la chaîne fautive ; les états d'artefact
 * `failed` servent de repli pour les serveurs qui ne renseignent pas `state`.
 */
export function datasetAuxIssues(dataset: HermesDatasetStatusLike): string[] {
  const state = dataset.state ?? ''
  const issues: string[] = []
  if (
    state === 'degradedCommunes' ||
    datasetArtifactState(dataset, 'communes') === 'failed'
  ) {
    issues.push('Communes')
  }
  if (
    state === 'degradedTunnels' ||
    datasetArtifactState(dataset, 'tunnels') === 'failed'
  ) {
    issues.push('Tunnels')
  }
  if (
    state === 'degradedAux' ||
    datasetArtifactState(dataset, 'mm') === 'failed'
  ) {
    issues.push('f(Tunnels/mm)')
  }
  return issues
}

/** Couleur du chip pour l'état d'un dataset (états v1.9 + repli `status`). */
export function datasetStateColor(dataset: HermesDatasetStatusLike): string {
  const state = dataset.state ?? ''
  if (state === 'ready') return 'green'
  if (state === 'unavailable') return 'red'
  if (state.startsWith('degraded')) return 'orange'
  // Initialisation et construction : bleu, comme les états « en cours » du
  // service.
  if (state === 'building' || state === 'initializing') return 'blue'
  switch (dataset.status) {
    case 'ready':
      return 'green'
    case 'preparing':
      return 'blue'
    case 'failed':
      return 'red'
    default:
      return 'grey'
  }
}

/**
 * Décrit le chip « serveur + service » de la toolbar de l'Accueil.
 *
 * `state` décrit la **machine** (ping système) ; `apiOnline` décrit le
 * **service** (`GET /ping`). Un serveur joignable avec une API muette affiche
 * « Service arrêté » (orange). Le libellé du dataset est son nom d'affichage
 * s'il existe, sinon son identifiant.
 */
export function describeHermesStatus(
  state: HermesServerState,
  dataset: HermesDatasetStatusLike | null,
  lastError: string | null,
  baseUrl: string,
  apiOnline: boolean | null = null,
): HermesStatusDisplay {
  const tooltip = [
    `Serveur hermes : ${stateLabel(state)}.`,
    `URL : ${baseUrl}`,
  ]

  if (state === 'online') {
    const service =
      apiOnline === true
        ? 'joignable'
        : apiOnline === false
          ? 'arrêté (API injoignable)'
          : 'état inconnu'
    tooltip.push(`Service hermes : ${service}.`)
  }

  if (dataset) {
    const stale = dataset.stale ? ' — obsolète' : ''
    tooltip.push(
      `Jeu de données servi : ${datasetDisplayName(dataset)} (${datasetStateLabel(
        dataset,
      )}${stale}).`,
    )
  } else {
    tooltip.push(
      'Aucun jeu de données servi — choisissez-en un dans la liste.',
    )
  }
  if (lastError) {
    tooltip.push(`Dernière erreur : ${lastError}`)
  }
  tooltip.push('Cliquer pour ouvrir la vue Hermes.')

  if (state === 'waking') {
    return {
      icon: 'mdi-progress-clock',
      color: 'orange',
      label: 'Réveil…',
      tooltip,
    }
  }
  if (state === 'offline') {
    return {
      icon: 'mdi-server-off',
      color: 'red',
      label: 'Hors ligne',
      tooltip,
    }
  }
  if (state === 'lost') {
    return {
      icon: 'mdi-server-off',
      color: 'red',
      label: 'Serveur perdu',
      tooltip,
    }
  }
  if (state === 'unknown') {
    return {
      icon: 'mdi-server',
      color: 'grey',
      label: 'Hermes',
      tooltip,
    }
  }
  // Machine joignable mais API muette : c'est le service qui est arrêté.
  if (state === 'online' && apiOnline === false) {
    return {
      icon: 'mdi-server-alert',
      color: 'orange',
      label: 'Service arrêté',
      tooltip,
    }
  }
  if (!dataset) {
    return {
      icon: 'mdi-server-network',
      color: 'orange',
      label: 'Jeu de données à choisir',
      tooltip,
    }
  }

  const color = datasetStateColor(dataset)
  if (color === 'green') {
    return {
      icon: 'mdi-server-network',
      color,
      label: datasetDisplayName(dataset),
      tooltip,
    }
  }
  return {
    icon: color === 'red' ? 'mdi-server-alert' : 'mdi-progress-clock',
    color,
    label: `${datasetDisplayName(dataset)} · ${datasetStateLabel(dataset)}`,
    tooltip,
  }
}

/**
 * Paramètres `Hermes.Limites.*` qui bornent la création des polygones et des
 * datasets. Les valeurs de repli ne servent que tant que les settings ne sont
 * pas chargés. La surface d'un dataset est **distincte** de celle d'un
 * polygone : la première borne la somme des zones assemblées, la seconde borne
 * chaque polygone pris isolément.
 */
export const HERMES_LIMIT_PATHS = {
  maxPolygonsPerDataset: 'Hermes.Limites.maxPolygonesParDataset',
  maxPolygonAreaKm2: 'Hermes.Limites.surfaceMaxPolygoneKm2',
  maxDatasetAreaKm2: 'Hermes.Limites.surfaceMaxDatasetKm2',
} as const

export const HERMES_LIMIT_DEFAULTS = {
  maxPolygonsPerDataset: 2,
  maxPolygonAreaKm2: 10_000,
  maxDatasetAreaKm2: 50_000,
} as const

/**
 * Génère un identifiant de polygone à partir d'un nom : minuscules, sans
 * accents, séparateurs `-`, préfixe `ma-zone-`. La longueur totale reste sous
 * la limite serveur de 64 caractères.
 */
export function slugifyPolygonName(name: string): string {
  const slug = name
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .replace(/^ma-zone-/, '')
    .slice(0, 52)
  return slug ? `ma-zone-${slug}` : ''
}

/** Identifiant de polygone valide : slug ASCII, `index` réservé. */
export function isValidPolygonSlug(id: string): boolean {
  return /^[A-Za-z0-9_-]{1,64}$/.test(id) && id !== 'index'
}

/** Aire sphérique d'un anneau, en km² (formule de Turf `ringArea`). */
function ringAreaKm2(ring: number[][]): number {
  if (ring.length < 3) return 0
  const earthRadiusKm = 6371.0088
  let total = 0
  for (let index = 0; index < ring.length; index += 1) {
    const [lon1, lat1] = ring[index]
    const [lon2, lat2] = ring[(index + 1) % ring.length]
    total +=
      toRadians(lon2 - lon1) *
      (2 + Math.sin(toRadians(lat1)) + Math.sin(toRadians(lat2)))
  }
  return (total * earthRadiusKm * earthRadiusKm) / 2
}

/**
 * Aire d'un polygone GeoJSON en km². Les trous éventuels sont soustraits ; le
 * serveur les refuse, mais la mesure reste juste pendant l'édition.
 */
export function polygonAreaKm2(geometry: GeoJSON.Polygon): number {
  const rings = geometry.coordinates ?? []
  if (rings.length === 0) return 0
  const outer = Math.abs(ringAreaKm2(rings[0]))
  const holes = rings
    .slice(1)
    .reduce((sum, ring) => sum + Math.abs(ringAreaKm2(ring)), 0)
  return Math.max(0, outer - holes)
}

/** Résultat de la validation locale d'une géométrie de polygone. */
export interface PolygonValidation {
  ok: boolean
  message: string | null
}

/**
 * Valide une géométrie avant envoi : `Polygon` uniquement, sans trous, anneau
 * fermé, 4 à 1000 sommets, coordonnées WGS84 finies.
 *
 * Le serveur arrondit à 5 décimales et ferme silencieusement un anneau ouvert ;
 * l'IHM est plus stricte pour signaler tout de suite une géométrie inattendue.
 */
export function validatePolygonGeometry(
  geometry: GeoJSON.Geometry | null | undefined,
): PolygonValidation {
  if (!geometry) {
    return { ok: false, message: 'Aucune géométrie à enregistrer.' }
  }
  if (geometry.type !== 'Polygon') {
    return {
      ok: false,
      message: 'La géométrie doit être un Polygon (MultiPolygon non accepté).',
    }
  }
  const rings = geometry.coordinates
  if (!Array.isArray(rings) || rings.length === 0) {
    return { ok: false, message: 'Le secteur est vide.' }
  }
  if (rings.length > 1) {
    return { ok: false, message: 'Les trous ne sont pas acceptés par le serveur.' }
  }
  const ring = rings[0]
  if (!Array.isArray(ring) || ring.length < 4) {
    return {
      ok: false,
      message: 'Un secteur doit avoir au moins 4 sommets (anneau fermé).',
    }
  }
  if (ring.length > 1000) {
    return { ok: false, message: 'Un secteur est limité à 1000 sommets.' }
  }
  const first = ring[0]
  const last = ring[ring.length - 1]
  if (!first || !last) {
    return { ok: false, message: 'Anneau de secteur invalide.' }
  }
  if (first[0] !== last[0] || first[1] !== last[1]) {
    return {
      ok: false,
      message: "L'anneau doit être fermé (premier et dernier point identiques).",
    }
  }
  for (const point of ring) {
    const [lon, lat] = point
    if (
      !Number.isFinite(lon) ||
      !Number.isFinite(lat) ||
      lon < -180 ||
      lon > 180 ||
      lat < -90 ||
      lat > 90
    ) {
      return {
        ok: false,
        message: 'Coordonnées invalides : WGS84 (lon ±180, lat ±90) attendu.',
      }
    }
  }
  return { ok: true, message: null }
}

/** Formate une bbox `[lon_min, lat_min, lon_max, lat_max]` pour l'affichage. */
export function formatBbox(
  bbox: [number, number, number, number],
): string {
  const [minLon, minLat, maxLon, maxLat] = bbox
  return `${minLon.toFixed(2)}, ${minLat.toFixed(2)} → ${maxLon.toFixed(2)}, ${maxLat.toFixed(2)}`
}

/** Union de plusieurs bbox, ou `null` si la liste est vide. */
export function unionBboxes(
  bboxes: [number, number, number, number][],
): [number, number, number, number] | null {
  if (bboxes.length === 0) return null
  let minLon = Number.POSITIVE_INFINITY
  let minLat = Number.POSITIVE_INFINITY
  let maxLon = Number.NEGATIVE_INFINITY
  let maxLat = Number.NEGATIVE_INFINITY
  for (const bbox of bboxes) {
    minLon = Math.min(minLon, bbox[0])
    minLat = Math.min(minLat, bbox[1])
    maxLon = Math.max(maxLon, bbox[2])
    maxLat = Math.max(maxLat, bbox[3])
  }
  if (!Number.isFinite(minLon) || !Number.isFinite(minLat)) return null
  return [minLon, minLat, maxLon, maxLat]
}

/** Formate une durée en secondes pour les progressions (build, réveil). */
export function formatSeconds(value: number): string {
  if (!Number.isFinite(value) || value < 0) return '—'
  if (value < 60) return `${Math.round(value)} s`
  const minutes = Math.floor(value / 60)
  const seconds = Math.round(value % 60)
  return `${minutes} min ${String(seconds).padStart(2, '0')} s`
}
