// Contrat de rendu de la carte Hermes : sources, couches, styles et
// conversions GeoJSON des datasets, régions et polygones custom.
//
// La carte est pilotée par l'onglet actif du panneau :
// - **Datasets** : dataset actif en vert, autres datasets en gris ;
// - **Régions** : contour du dataset actif en vert, régions en jaune ;
// - **Polygones** : contour du dataset actif en vert, polygones en orange.
//
// Dans tous les cas, l'élément survolé (ou sélectionné) passe en bleu. Les
// couleurs sont des nuances de la palette Material Design.
//
// Les données sont rechargées à chaque changement (FeatureCollections) sans
// recréer les couches ; seule leur visibilité dépend de l'onglet.

import type {
  ExpressionSpecification,
  FillLayerSpecification,
  GeoJSONSource,
  LineLayerSpecification,
  Map,
} from 'mapbox-gl'
import type { HermesTab } from '../../utils/hermes'
import {
  datasetGeometries,
  regionGeometry,
  type DatasetScope,
  type PolygonShape,
  type RegionShape,
} from '../../utils/hermesGeometry'

export const HERMES_MAP_STYLE = 'mapbox://styles/mapbox/outdoors-v12'

export const HERMES_SOURCE_IDS = {
  coverage: 'hermes-coverage',
  datasets: 'hermes-datasets',
  regions: 'hermes-regions',
  polygons: 'hermes-polygons',
} as const

export const HERMES_LAYER_IDS = {
  coverageFill: 'hermes-coverage-fill',
  coverageLine: 'hermes-coverage-line',
  datasetsFill: 'hermes-datasets-fill',
  datasetsLine: 'hermes-datasets-line',
  regionsFill: 'hermes-regions-fill',
  regionsLine: 'hermes-regions-line',
  polygonsFill: 'hermes-polygons-fill',
  polygonsLine: 'hermes-polygons-line',
} as const

/**
 * Palette Material Design (nuances 500 pour les remplissages, 800/900 pour
 * les traits). Le bleu marque le survol et la sélection.
 */
export const HERMES_MAP_COLORS = {
  activeFill: '#4CAF50',
  activeLine: '#2E7D32',
  otherFill: '#9E9E9E',
  otherLine: '#757575',
  highlightFill: '#2196F3',
  highlightLine: '#0D47A1',
  regionFill: '#FFC107',
  regionLine: '#F9A825',
  polygonFill: '#FF9800',
  polygonLine: '#EF6C00',
} as const

export function emptyFC(): GeoJSON.FeatureCollection {
  return { type: 'FeatureCollection', features: [] }
}

/** Couleur selon la propriété booléenne `highlight`. */
function stateColor(
  highlighted: string,
  normal: string,
): ExpressionSpecification {
  return [
    'case',
    ['==', ['get', 'highlight'], true],
    highlighted,
    normal,
  ] as unknown as ExpressionSpecification
}

/** Opacité de remplissage selon la propriété booléenne `highlight`. */
function stateOpacity(
  highlighted: number,
  normal: number,
): ExpressionSpecification {
  return [
    'case',
    ['==', ['get', 'highlight'], true],
    highlighted,
    normal,
  ] as unknown as ExpressionSpecification
}

/** Épaisseur de trait selon la propriété booléenne `highlight`. */
function stateWidth(
  highlighted: number,
  normal: number,
): ExpressionSpecification {
  return [
    'case',
    ['==', ['get', 'highlight'], true],
    highlighted,
    normal,
  ] as unknown as ExpressionSpecification
}

/** Couverture du dataset actif (vert, ou bleu s'il est survolé). */
export function coverageFeatureCollection(
  dataset: DatasetScope,
  regions: RegionShape[],
  polygons: PolygonShape[],
  highlighted: boolean,
): GeoJSON.FeatureCollection {
  return {
    type: 'FeatureCollection',
    features: datasetGeometries(dataset, regions, polygons).map(
      (geometry): GeoJSON.Feature => ({
        type: 'Feature',
        properties: { highlight: highlighted },
        geometry,
      }),
    ),
  }
}

/** Couverture des autres datasets, chacun identifiable pour le survol. */
export function datasetsFeatureCollection(
  datasets: DatasetScope[],
  regions: RegionShape[],
  polygons: PolygonShape[],
  highlightedId: string | null,
): GeoJSON.FeatureCollection {
  const features: GeoJSON.Feature[] = []
  for (const dataset of datasets) {
    const highlighted = dataset.id === highlightedId
    for (const geometry of datasetGeometries(dataset, regions, polygons)) {
      features.push({
        type: 'Feature',
        properties: { id: dataset.id, highlight: highlighted },
        geometry,
      })
    }
  }
  return { type: 'FeatureCollection', features }
}

/** Régions affichables ; `highlight` couvre sélection, aperçu et survol. */
export function regionsFeatureCollection(
  regions: RegionShape[],
  selectedId: string | null,
  previewIds: string[] = [],
  hoveredId: string | null = null,
): GeoJSON.FeatureCollection {
  const preview = new Set(previewIds)
  const features: GeoJSON.Feature[] = regions.map(
    (region): GeoJSON.Feature => ({
      type: 'Feature',
      properties: {
        id: region.id,
        highlight:
          region.id === selectedId ||
          preview.has(region.id) ||
          region.id === hoveredId,
      },
      geometry: regionGeometry(region),
    }),
  )
  return { type: 'FeatureCollection', features }
}

/**
 * Polygones custom affichables.
 *
 * `excludeId` retire le polygone en cours d'édition : Mapbox Draw le dessine
 * déjà avec ses propres couches, un doublon décalerait les contours.
 */
export function polygonsFeatureCollection(
  polygons: PolygonShape[],
  selectedId: string | null,
  previewIds: string[] = [],
  hoveredId: string | null = null,
  excludeId: string | null = null,
): GeoJSON.FeatureCollection {
  const preview = new Set(previewIds)
  const features: GeoJSON.Feature[] = polygons
    .filter(polygon => polygon.id !== excludeId)
    .map(
      (polygon): GeoJSON.Feature => ({
        type: 'Feature',
        properties: {
          id: polygon.id,
          highlight:
            polygon.id === selectedId ||
            preview.has(polygon.id) ||
            polygon.id === hoveredId,
        },
        geometry: polygon.geometry,
      }),
    )
  return { type: 'FeatureCollection', features }
}

export function setSourceData(
  map: Map,
  sourceId: string,
  data: GeoJSON.FeatureCollection,
): void {
  const source = map.getSource(sourceId) as GeoJSONSource | undefined
  source?.setData(data)
}

/** Rend visibles les seules couches utiles à l'onglet courant. */
export function applyHermesVisibility(
  map: Map,
  tab: HermesTab,
  hasRegionPreview: boolean,
  hasPolygonPreview: boolean,
): void {
  setLayerVisibility(map, [HERMES_LAYER_IDS.coverageFill], tab === 'datasets')
  // Le contour vert reste visible sur tous les onglets : il matérialise le
  // périmètre du dataset actif sans masquer les remplissages jaune/orange.
  setLayerVisibility(map, [HERMES_LAYER_IDS.coverageLine], true)
  setLayerVisibility(
    map,
    [HERMES_LAYER_IDS.datasetsFill, HERMES_LAYER_IDS.datasetsLine],
    tab === 'datasets',
  )
  setLayerVisibility(
    map,
    [HERMES_LAYER_IDS.regionsFill, HERMES_LAYER_IDS.regionsLine],
    tab === 'regions' || hasRegionPreview,
  )
  setLayerVisibility(
    map,
    [HERMES_LAYER_IDS.polygonsFill, HERMES_LAYER_IDS.polygonsLine],
    tab === 'polygons' || hasPolygonPreview,
  )
}

function setLayerVisibility(
  map: Map,
  layerIds: string[],
  visible: boolean,
): void {
  const visibility = visible ? 'visible' : 'none'
  for (const layerId of layerIds) {
    if (map.getLayer(layerId)) {
      map.setLayoutProperty(layerId, 'visibility', visibility)
    }
  }
}

/** Ajoute sources et couches si elles n'existent pas encore. */
export function addHermesLayers(map: Map): void {
  addSourceIfMissing(map, HERMES_SOURCE_IDS.coverage)
  addSourceIfMissing(map, HERMES_SOURCE_IDS.datasets)
  addSourceIfMissing(map, HERMES_SOURCE_IDS.regions)
  addSourceIfMissing(map, HERMES_SOURCE_IDS.polygons)

  // Ordre de superposition : les remplissages d'abord, le contour vert du
  // dataset au-dessus d'eux, puis les traits de sélection/survol.
  if (!map.getLayer(HERMES_LAYER_IDS.coverageFill)) {
    const coverageFill: FillLayerSpecification = {
      id: HERMES_LAYER_IDS.coverageFill,
      type: 'fill',
      source: HERMES_SOURCE_IDS.coverage,
      paint: {
        'fill-color': stateColor(
          HERMES_MAP_COLORS.highlightFill,
          HERMES_MAP_COLORS.activeFill,
        ),
        'fill-opacity': stateOpacity(0.35, 0.28),
      },
    }
    map.addLayer(coverageFill)
  }

  if (!map.getLayer(HERMES_LAYER_IDS.datasetsFill)) {
    const datasetsFill: FillLayerSpecification = {
      id: HERMES_LAYER_IDS.datasetsFill,
      type: 'fill',
      source: HERMES_SOURCE_IDS.datasets,
      paint: {
        'fill-color': stateColor(
          HERMES_MAP_COLORS.highlightFill,
          HERMES_MAP_COLORS.otherFill,
        ),
        'fill-opacity': stateOpacity(0.35, 0.1),
      },
    }
    map.addLayer(datasetsFill)
  }

  if (!map.getLayer(HERMES_LAYER_IDS.datasetsLine)) {
    const datasetsLine: LineLayerSpecification = {
      id: HERMES_LAYER_IDS.datasetsLine,
      type: 'line',
      source: HERMES_SOURCE_IDS.datasets,
      paint: {
        'line-color': stateColor(
          HERMES_MAP_COLORS.highlightLine,
          HERMES_MAP_COLORS.otherLine,
        ),
        'line-width': stateWidth(2.5, 1),
      },
    }
    map.addLayer(datasetsLine)
  }

  if (!map.getLayer(HERMES_LAYER_IDS.regionsFill)) {
    const regionsFill: FillLayerSpecification = {
      id: HERMES_LAYER_IDS.regionsFill,
      type: 'fill',
      source: HERMES_SOURCE_IDS.regions,
      paint: {
        'fill-color': stateColor(
          HERMES_MAP_COLORS.highlightFill,
          HERMES_MAP_COLORS.regionFill,
        ),
        'fill-opacity': stateOpacity(0.4, 0.18),
      },
    }
    map.addLayer(regionsFill)
  }

  if (!map.getLayer(HERMES_LAYER_IDS.polygonsFill)) {
    const polygonsFill: FillLayerSpecification = {
      id: HERMES_LAYER_IDS.polygonsFill,
      type: 'fill',
      source: HERMES_SOURCE_IDS.polygons,
      paint: {
        'fill-color': stateColor(
          HERMES_MAP_COLORS.highlightFill,
          HERMES_MAP_COLORS.polygonFill,
        ),
        'fill-opacity': stateOpacity(0.4, 0.18),
      },
    }
    map.addLayer(polygonsFill)
  }

  if (!map.getLayer(HERMES_LAYER_IDS.coverageLine)) {
    const coverageLine: LineLayerSpecification = {
      id: HERMES_LAYER_IDS.coverageLine,
      type: 'line',
      source: HERMES_SOURCE_IDS.coverage,
      paint: {
        'line-color': stateColor(
          HERMES_MAP_COLORS.highlightLine,
          HERMES_MAP_COLORS.activeLine,
        ),
        'line-width': stateWidth(3, 2.5),
      },
    }
    map.addLayer(coverageLine)
  }

  if (!map.getLayer(HERMES_LAYER_IDS.regionsLine)) {
    const regionsLine: LineLayerSpecification = {
      id: HERMES_LAYER_IDS.regionsLine,
      type: 'line',
      source: HERMES_SOURCE_IDS.regions,
      paint: {
        'line-color': stateColor(
          HERMES_MAP_COLORS.highlightLine,
          HERMES_MAP_COLORS.regionLine,
        ),
        'line-width': stateWidth(2.5, 1),
      },
    }
    map.addLayer(regionsLine)
  }

  if (!map.getLayer(HERMES_LAYER_IDS.polygonsLine)) {
    const polygonsLine: LineLayerSpecification = {
      id: HERMES_LAYER_IDS.polygonsLine,
      type: 'line',
      source: HERMES_SOURCE_IDS.polygons,
      paint: {
        'line-color': stateColor(
          HERMES_MAP_COLORS.highlightLine,
          HERMES_MAP_COLORS.polygonLine,
        ),
        'line-width': stateWidth(2.5, 1),
      },
    }
    map.addLayer(polygonsLine)
  }
}

function addSourceIfMissing(map: Map, sourceId: string): void {
  if (!map.getSource(sourceId)) {
    map.addSource(sourceId, { type: 'geojson', data: emptyFC() })
  }
}
