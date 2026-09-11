// Registres de couches Mapbox de la vue Audit (IHM §3.4 et §3.5).
//
// Mapbox GL n'a pas de « panes » nommés : l'empilement est déterminé par
// l'**ordre d'ajout** des couches (bas → haut). Toutes les couches sont donc
// créées une seule fois au `load`, et leur donnée est ensuite mise à jour par
// `source.setData()`.
//
// Les cinq registres remplacent les `layerGroup` du HTML de référence ; le
// contrat de `remove()` (retirer le layer **avant** la source, sinon Mapbox
// refuse) et l'idempotence de `add()` sont conservés tels quels.

import type {
  GeoJSONSource,
  LayerSpecification,
  Map,
  MapLayerMouseEvent,
} from 'mapbox-gl'

/** Collection de features vide (source neutre avant tout rendu). */
export function emptyFC(): GeoJSON.FeatureCollection {
  return { type: 'FeatureCollection', features: [] }
}

/**
 * Palette du module Audit — table chromatique de l'IHM §15.
 *
 * Ces valeurs sont un **contrat de rendu** (elles identifient les éléments de
 * l'anomalie sur la carte), pas la palette d'interface de l'application : le
 * chrome de la vue reste en Vuetify.
 */
export const AUDIT_COLORS = {
  trace: '#3564e0',
  warn: '#dd3327',
  ctx: '#3ecf8e',
  fp: '#6ea8ff',
  orig: '#8a919c',
  applied: '#6ea8ff',
  selA: '#f0d24a',
  selB: '#ec9c11',
  anchorA: '#c084fc',
  anchorB: '#7e22ce',
  routeCar: '#f0d24a',
  routeBike: '#ec9c11',
  delDrop: '#d9dde3',
  delKeep: '#8fc1ff',
  delJoin: '#3564e0',
  labelLeader: '#9aa2af',
  kasasCross: '#f0d24a',
} as const

/** Style de carte et vue de repli (IHM §19.6). */
export const AUDIT_MAP_STYLE = 'mapbox://styles/mapbox/outdoors-v12'
export const AUDIT_MAP_CENTER: [number, number] = [2.6, 46.6]
export const AUDIT_MAP_ZOOM = 6

/** Identifiants des couches, dans l'ordre d'empilement (bas → haut). */
export const LAYER_IDS = {
  applied: 'lyr-applied',
  trace: 'lyr-trace',
  traceEnds: 'lyr-trace-ends',
  previewCar: 'lyr-preview-car',
  previewCarPts: 'lyr-preview-car-pts',
  previewBike: 'lyr-preview-bike',
  previewBikePts: 'lyr-preview-bike-pts',
  anomalyLines: 'lyr-anomaly-lines',
  anomalyPoints: 'lyr-anomaly-points',
  delRed: 'lyr-del-red',
  delJoin: 'lyr-del-join',
  labelLeaders: 'lyr-label-leaders',
} as const

/**
 * Registre de couches : une `Map` interne `id → { source, layer }` et une API
 * minimale (`add`, `update`, `remove`, `clear`).
 */
export class LayerRegistry {
  private map: Map | null = null
  private names = new Set<string>()

  /** Attache (ou détache avec `null`) la carte du registre. */
  attach(map: Map | null): void {
    if (map === null) this.names.clear()
    this.map = map
  }

  /** `true` si le registre contient une couche de cet identifiant. */
  has(id: string): boolean {
    return this.names.has(id)
  }

  /**
   * Crée la source et la couche, ou met à jour la donnée si l'identifiant est
   * déjà présent (idempotence : un double appel de rendu est sans effet de bord).
   */
  add(
    id: string,
    fc: GeoJSON.FeatureCollection,
    layerSpec: Omit<LayerSpecification, 'id' | 'source'>,
    onClick?: (e: MapLayerMouseEvent) => void,
  ): void {
    const map = this.map
    if (!map) return
    if (map.getSource(id)) {
      this.update(id, fc)
      return
    }
    map.addSource(id, { type: 'geojson', data: fc })
    map.addLayer({ id, source: id, ...layerSpec } as LayerSpecification)
    this.names.add(id)
    if (onClick) map.on('click', id, onClick)
  }

  /** Met à jour la donnée (no-op si la source est absente). */
  update(id: string, fc: GeoJSON.FeatureCollection): void {
    const src = this.map?.getSource(id) as GeoJSONSource | undefined
    if (src) src.setData(fc)
  }

  /** Retire la couche **puis** la source — cet ordre est obligatoire. */
  remove(id: string): void {
    const map = this.map
    if (!map) return
    if (map.getLayer(id)) map.removeLayer(id)
    if (map.getSource(id)) map.removeSource(id)
    this.names.delete(id)
  }

  /** Retire tous les éléments du registre. */
  clear(): void {
    for (const id of [...this.names]) this.remove(id)
  }
}

/** Les cinq registres fonctionnels de la vue Audit (IHM §3.5). */
export interface AuditRegistries {
  trace: LayerRegistry
  applied: LayerRegistry
  preview: LayerRegistry
  anomaly: LayerRegistry
  del: LayerRegistry
}

/** Crée un jeu de registres neuf (détaché de toute carte). */
export function createRegistries(): AuditRegistries {
  return {
    trace: new LayerRegistry(),
    applied: new LayerRegistry(),
    preview: new LayerRegistry(),
    anomaly: new LayerRegistry(),
    del: new LayerRegistry(),
  }
}

/** Attache les cinq registres à la carte. */
export function attachRegistries(regs: AuditRegistries, map: Map): void {
  regs.trace.attach(map)
  regs.applied.attach(map)
  regs.preview.attach(map)
  regs.anomaly.attach(map)
  regs.del.attach(map)
}

/** Détache les cinq registres (destruction de la carte). */
export function detachRegistries(regs: AuditRegistries): void {
  regs.trace.attach(null)
  regs.applied.attach(null)
  regs.preview.attach(null)
  regs.anomaly.attach(null)
  regs.del.attach(null)
}

/** Affiche les couches cliquables et gère le curseur `pointer` au survol. */
const CLICKABLE_IDS: string[] = [
  LAYER_IDS.applied,
  LAYER_IDS.traceEnds,
  LAYER_IDS.anomalyLines,
  LAYER_IDS.anomalyPoints,
]

/**
 * Crée les sources et couches de la carte, dans l'ordre d'empilement de l'IHM
 * §3.4 (1 = le plus bas). À appeler au `load` de la carte.
 *
 * `showPopupAt` reçoit le HTML porté par la feature cliquée (`popupHtml`) : un
 * seul popup est actif à la fois, aucune feature ne porte de popup en propre.
 */
export function initAuditLayers(
  map: Map,
  regs: AuditRegistries,
  showPopupAt: (e: MapLayerMouseEvent) => void,
): void {
  // 1. Routage appliqué — sous la trace.
  regs.applied.add(
    LAYER_IDS.applied,
    emptyFC(),
    {
      type: 'line',
      paint: {
        'line-color': AUDIT_COLORS.applied,
        'line-width': 8,
        'line-opacity': 0.9,
      },
      layout: { 'line-cap': 'round', 'line-join': 'round' },
    },
    showPopupAt,
  )

  // 2. Trace de base.
  regs.trace.add(LAYER_IDS.trace, emptyFC(), {
    type: 'line',
    paint: {
      'line-color': AUDIT_COLORS.trace,
      'line-width': 3,
      'line-opacity': 0.95,
    },
    layout: { 'line-cap': 'round', 'line-join': 'round' },
  })

  // 3. Marqueurs Départ / Arrivée.
  regs.trace.add(
    LAYER_IDS.traceEnds,
    emptyFC(),
    {
      type: 'circle',
      paint: {
        'circle-radius': 4.5,
        'circle-color': ['case', ['==', ['get', 'kind'], 'start'], '#ffffff', '#20242b'],
        'circle-stroke-color': '#20242b',
        'circle-stroke-width': 2,
      },
    },
    showPopupAt,
  )

  // 4-5. Aperçu de routage voiture.
  regs.preview.add(LAYER_IDS.previewCar, emptyFC(), {
    type: 'line',
    paint: {
      'line-color': AUDIT_COLORS.routeCar,
      'line-width': 10,
      'line-opacity': 0.9,
    },
    layout: { 'line-cap': 'round', 'line-join': 'round' },
  })
  regs.preview.add(LAYER_IDS.previewCarPts, emptyFC(), {
    type: 'circle',
    paint: {
      'circle-radius': 4,
      'circle-color': AUDIT_COLORS.routeCar,
      'circle-stroke-color': '#ffffff',
      'circle-stroke-width': 1.2,
    },
  })

  // 6-7. Aperçu de routage vélo de route.
  regs.preview.add(LAYER_IDS.previewBike, emptyFC(), {
    type: 'line',
    paint: {
      'line-color': AUDIT_COLORS.routeBike,
      'line-width': 6,
      'line-opacity': 0.9,
    },
    layout: { 'line-cap': 'round', 'line-join': 'round' },
  })
  regs.preview.add(LAYER_IDS.previewBikePts, emptyFC(), {
    type: 'circle',
    paint: {
      'circle-radius': 3,
      'circle-color': AUDIT_COLORS.routeBike,
      'circle-stroke-color': '#ffffff',
      'circle-stroke-width': 1.2,
    },
  })

  // 8-9. Traits et points des anomalies — data-driven : un layer unique porte
  // des features hétérogènes, chacune décrivant sa couleur et sa taille.
  regs.anomaly.add(
    LAYER_IDS.anomalyLines,
    emptyFC(),
    {
      type: 'line',
      paint: {
        'line-color': ['get', 'color'],
        'line-width': ['get', 'width'],
        'line-opacity': ['get', 'opacity'],
      },
      layout: { 'line-cap': 'round', 'line-join': 'round' },
    },
    showPopupAt,
  )
  regs.anomaly.add(
    LAYER_IDS.anomalyPoints,
    emptyFC(),
    {
      type: 'circle',
      paint: {
        'circle-radius': ['get', 'radius'],
        'circle-color': ['get', 'fill'],
        'circle-stroke-color': ['get', 'stroke'],
        'circle-stroke-width': ['get', 'strokeWidth'],
      },
    },
    showPopupAt,
  )

  // 10-11. Prévisualisation de suppression — le rouge passe **sous** le bleu.
  regs.del.add(LAYER_IDS.delRed, emptyFC(), {
    type: 'line',
    paint: {
      'line-color': AUDIT_COLORS.warn,
      'line-width': 5,
      'line-opacity': 0.95,
    },
    layout: { 'line-cap': 'round', 'line-join': 'round' },
  })
  regs.del.add(LAYER_IDS.delJoin, emptyFC(), {
    type: 'line',
    paint: {
      'line-color': AUDIT_COLORS.delJoin,
      'line-width': 4,
      'line-opacity': 0.95,
    },
    layout: { 'line-cap': 'round', 'line-join': 'round' },
  })

  // 12. Traits de liaison des étiquettes.
  map.addSource(LAYER_IDS.labelLeaders, { type: 'geojson', data: emptyFC() })
  map.addLayer({
    id: LAYER_IDS.labelLeaders,
    source: LAYER_IDS.labelLeaders,
    type: 'line',
    paint: {
      'line-color': ['get', 'color'],
      'line-width': 2,
      'line-opacity': 0.9,
    },
  })

  // Curseur main sur les couches interactives.
  const setPointer = () => {
    map.getCanvas().style.cursor = 'pointer'
  }
  const clearPointer = () => {
    map.getCanvas().style.cursor = ''
  }
  for (const id of CLICKABLE_IDS) {
    map.on('mouseenter', id, setPointer)
    map.on('mouseleave', id, clearPointer)
  }
}

/** Publie les traits de liaison (les étiquettes masquées sont filtrées). */
export function publishLabelLeaders(
  map: Map | null,
  features: GeoJSON.Feature[],
): void {
  const src = map?.getSource(LAYER_IDS.labelLeaders) as
    | GeoJSONSource
    | undefined
  if (!src) return
  src.setData({
    type: 'FeatureCollection',
    features: features.filter((f) => Number(f.properties?.opacity ?? 0) > 0),
  })
}
