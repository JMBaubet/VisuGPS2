// Contrat de rendu de la carte Hermes : sources, couches et conversions
// GeoJSON des régions et des polygones custom.
//
// La carte affiche deux familles de données :
// - les **régions** indexées par l'administrateur (`GET /regions`), en vert,
//   avec leur contour réel ou le rectangle de leur bbox ;
// - les **polygones custom** (`GET /polygons`), remplis de leur couleur.
//
// La sélection est portée par une propriété `selected` : le composant carte
// recharge les FeatureCollections à chaque changement, sans recréer les
// couches.

import type {
  ExpressionSpecification,
  FillLayerSpecification,
  GeoJSONSource,
  LineLayerSpecification,
  Map,
} from 'mapbox-gl'
import type { HermesPolygon, HermesRegion } from '../../stores/hermes'

export const HERMES_MAP_STYLE = 'mapbox://styles/mapbox/outdoors-v12'

export const HERMES_SOURCE_IDS = {
  regions: 'hermes-regions',
  polygons: 'hermes-polygons',
} as const

export const HERMES_LAYER_IDS = {
  regionsFill: 'hermes-regions-fill',
  regionsLine: 'hermes-regions-line',
  polygonsFill: 'hermes-polygons-fill',
  polygonsLine: 'hermes-polygons-line',
} as const

const DEFAULT_POLYGON_COLOR = '#3B82F6'

export function emptyFC(): GeoJSON.FeatureCollection {
  return { type: 'FeatureCollection', features: [] }
}

/** Rectangle GeoJSON d'une bbox `[lon_min, lat_min, lon_max, lat_max]`. */
export function bboxPolygon(
  bbox: [number, number, number, number],
): GeoJSON.Polygon {
  const [minLon, minLat, maxLon, maxLat] = bbox
  return {
    type: 'Polygon',
    coordinates: [
      [
        [minLon, minLat],
        [maxLon, minLat],
        [maxLon, maxLat],
        [minLon, maxLat],
        [minLon, minLat],
      ],
    ],
  }
}

/**
 * Géométrie affichable d'une région : son contour réel s'il est un polygone,
 * sinon le rectangle de sa bbox (`has_poly=false`).
 */
function regionGeometry(region: HermesRegion): GeoJSON.Geometry {
  const geometry = region.geojson
  if (
    geometry &&
    (geometry.type === 'Polygon' || geometry.type === 'MultiPolygon')
  ) {
    return geometry
  }
  return bboxPolygon(region.bbox)
}

export function regionsFeatureCollection(
  regions: HermesRegion[],
  selectedId: string | null,
  previewIds: string[] = [],
): GeoJSON.FeatureCollection {
  const preview = new Set(previewIds)
  const features: GeoJSON.Feature[] = regions.map(
    (region): GeoJSON.Feature => ({
      type: 'Feature',
      properties: {
        id: region.id,
        selected: region.id === selectedId || preview.has(region.id),
        hasPoly: region.hasPoly,
      },
      geometry: regionGeometry(region),
    }),
  )
  return { type: 'FeatureCollection', features }
}

export function polygonsFeatureCollection(
  polygons: HermesPolygon[],
  selectedId: string | null,
  previewIds: string[] = [],
): GeoJSON.FeatureCollection {
  const preview = new Set(previewIds)
  const features: GeoJSON.Feature[] = polygons.map(
    (polygon): GeoJSON.Feature => ({
      type: 'Feature',
      properties: {
        id: polygon.id,
        selected: polygon.id === selectedId || preview.has(polygon.id),
        color: polygon.color ?? DEFAULT_POLYGON_COLOR,
      },
      geometry: polygon.geometry,
    }),
  )
  return { type: 'FeatureCollection', features }
}

export function setRegionsData(
  map: Map,
  data: GeoJSON.FeatureCollection,
): void {
  const source = map.getSource(HERMES_SOURCE_IDS.regions) as
    | GeoJSONSource
    | undefined
  source?.setData(data)
}

export function setPolygonsData(
  map: Map,
  data: GeoJSON.FeatureCollection,
): void {
  const source = map.getSource(HERMES_SOURCE_IDS.polygons) as
    | GeoJSONSource
    | undefined
  source?.setData(data)
}

/** Ajoute sources et couches si elles n'existent pas encore. */
export function addHermesLayers(map: Map): void {
  if (!map.getSource(HERMES_SOURCE_IDS.regions)) {
    map.addSource(HERMES_SOURCE_IDS.regions, {
      type: 'geojson',
      data: emptyFC(),
    })
  }
  if (!map.getSource(HERMES_SOURCE_IDS.polygons)) {
    map.addSource(HERMES_SOURCE_IDS.polygons, {
      type: 'geojson',
      data: emptyFC(),
    })
  }

  if (!map.getLayer(HERMES_LAYER_IDS.regionsFill)) {
    const regionsFill: FillLayerSpecification = {
      id: HERMES_LAYER_IDS.regionsFill,
      type: 'fill',
      source: HERMES_SOURCE_IDS.regions,
      paint: {
        'fill-color': [
          'case',
          ['==', ['get', 'selected'], true],
          '#2196F3',
          '#4CAF50',
        ] as unknown as ExpressionSpecification,
        'fill-opacity': [
          'case',
          ['==', ['get', 'selected'], true],
          0.35,
          0.12,
        ] as unknown as ExpressionSpecification,
      },
    }
    map.addLayer(regionsFill)
  }

  if (!map.getLayer(HERMES_LAYER_IDS.regionsLine)) {
    const regionsLine: LineLayerSpecification = {
      id: HERMES_LAYER_IDS.regionsLine,
      type: 'line',
      source: HERMES_SOURCE_IDS.regions,
      paint: {
        'line-color': [
          'case',
          ['==', ['get', 'selected'], true],
          '#0D47A1',
          '#2E7D32',
        ] as unknown as ExpressionSpecification,
        'line-width': [
          'case',
          ['==', ['get', 'selected'], true],
          2.5,
          1,
        ] as unknown as ExpressionSpecification,
      },
    }
    map.addLayer(regionsLine)
  }

  if (!map.getLayer(HERMES_LAYER_IDS.polygonsFill)) {
    const polygonsFill: FillLayerSpecification = {
      id: HERMES_LAYER_IDS.polygonsFill,
      type: 'fill',
      source: HERMES_SOURCE_IDS.polygons,
      paint: {
        'fill-color': [
          'coalesce',
          ['get', 'color'],
          DEFAULT_POLYGON_COLOR,
        ] as unknown as ExpressionSpecification,
        'fill-opacity': [
          'case',
          ['==', ['get', 'selected'], true],
          0.45,
          0.15,
        ] as unknown as ExpressionSpecification,
      },
    }
    map.addLayer(polygonsFill)
  }

  if (!map.getLayer(HERMES_LAYER_IDS.polygonsLine)) {
    const polygonsLine: LineLayerSpecification = {
      id: HERMES_LAYER_IDS.polygonsLine,
      type: 'line',
      source: HERMES_SOURCE_IDS.polygons,
      paint: {
        'line-color': [
          'case',
          ['==', ['get', 'selected'], true],
          '#0D47A1',
          ['coalesce', ['get', 'color'], DEFAULT_POLYGON_COLOR],
        ] as unknown as ExpressionSpecification,
        'line-width': [
          'case',
          ['==', ['get', 'selected'], true],
          2.5,
          1,
        ] as unknown as ExpressionSpecification,
      },
    }
    map.addLayer(polygonsLine)
  }
}
