/**
 * Helpers géométriques purs de la carte Hermes.
 *
 * Ils reconstituent la couverture d'un dataset à partir des géométries déjà
 * chargées (régions et polygones) : aucun appel serveur supplémentaire. Une
 * région sans contour réel est représentée par le rectangle de sa bbox.
 *
 * Aucune dépendance UI ni Mapbox : le contrat de rendu de la carte et la vue
 * Accueil consomment ces fonctions.
 */

import { toRadians } from './geo'

/** Région minimale nécessaire au calcul d'une couverture. */
export interface RegionShape {
  id: string
  bbox: [number, number, number, number]
  geojson: GeoJSON.Geometry | null
}

/** Polygone custom minimal nécessaire au calcul d'une couverture. */
export interface PolygonShape {
  id: string
  geometry: GeoJSON.Polygon
}

/** Périmètre d'un dataset : régions XOR zones. */
export interface DatasetScope {
  id: string
  regions: string[] | null
  zones: string[] | null
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
 * sinon le rectangle de sa bbox (`has_poly=false` ou `geojson` absent).
 */
export function regionGeometry(region: RegionShape): GeoJSON.Geometry {
  const geometry = region.geojson
  if (
    geometry &&
    (geometry.type === 'Polygon' || geometry.type === 'MultiPolygon')
  ) {
    return geometry
  }
  return bboxPolygon(region.bbox)
}

/**
 * Géométries qui composent la couverture d'un dataset.
 *
 * Les identifiants inconnus sont ignorés : une région supprimée côté serveur,
 * ou un polygone supprimé, ne doit pas empêcher l'affichage du reste.
 */
export function datasetGeometries(
  dataset: DatasetScope,
  regions: RegionShape[],
  polygons: PolygonShape[],
): GeoJSON.Geometry[] {
  const geometries: GeoJSON.Geometry[] = []

  for (const regionId of dataset.regions ?? []) {
    const region = regions.find(candidate => candidate.id === regionId)
    if (region) geometries.push(regionGeometry(region))
  }
  for (const polygonId of dataset.zones ?? []) {
    const polygon = polygons.find(candidate => candidate.id === polygonId)
    if (polygon) geometries.push(polygon.geometry)
  }

  return geometries
}

// ─── Cercles de distance (outil « marqueur distance ») ────────────────

/** Pas, portée et renforcement des cercles de distance. */
export const DISTANCE_RING_OPTIONS = {
  stepKm: 10,
  maxKm: 150,
  majorEveryKm: 50,
} as const

const EARTH_RADIUS_KM = 6371.0088
const RING_SEGMENTS = 180

/**
 * Point situé à `distanceKm` du centre, dans la direction `bearingDeg`
 * (0 = nord, sens horaire). Formule sphérique directe.
 */
export function destinationPoint(
  center: [number, number],
  bearingDeg: number,
  distanceKm: number,
): [number, number] {
  const [lon, lat] = center
  const angular = distanceKm / EARTH_RADIUS_KM
  const bearing = toRadians(bearingDeg)
  const lat1 = toRadians(lat)
  const lon1 = toRadians(lon)

  const sinLat2 =
    Math.sin(lat1) * Math.cos(angular) +
    Math.cos(lat1) * Math.sin(angular) * Math.cos(bearing)
  const lat2 = Math.asin(Math.min(1, Math.max(-1, sinLat2)))
  const lon2 =
    lon1 +
    Math.atan2(
      Math.sin(bearing) * Math.sin(angular) * Math.cos(lat1),
      Math.cos(angular) - Math.sin(lat1) * Math.sin(lat2),
    )

  const lonDeg = (lon2 * 180) / Math.PI
  const normalizedLon = ((lonDeg + 540) % 360) - 180
  return [normalizedLon, (lat2 * 180) / Math.PI]
}

/** Anneau fermé d'un cercle géodésique. */
function ringCoordinates(
  center: [number, number],
  radiusKm: number,
): number[][] {
  const coordinates: number[][] = []
  for (let index = 0; index <= RING_SEGMENTS; index += 1) {
    const bearing = (index * 360) / RING_SEGMENTS
    coordinates.push(destinationPoint(center, bearing, radiusKm))
  }
  return coordinates
}

/** Cercles concentriques de 10 à 150 km, renforcés tous les 50 km. */
export function distanceRingsFeatureCollection(
  center: [number, number],
): GeoJSON.FeatureCollection {
  const features: GeoJSON.Feature[] = []
  for (
    let radius = DISTANCE_RING_OPTIONS.stepKm;
    radius <= DISTANCE_RING_OPTIONS.maxKm;
    radius += DISTANCE_RING_OPTIONS.stepKm
  ) {
    features.push({
      type: 'Feature',
      properties: {
        radiusKm: radius,
        major: radius % DISTANCE_RING_OPTIONS.majorEveryKm === 0,
      },
      geometry: {
        type: 'LineString',
        coordinates: ringCoordinates(center, radius),
      },
    })
  }
  return { type: 'FeatureCollection', features }
}

/** Étiquette d'un cercle renforcé, positionnée au nord. */
export interface DistanceRingLabel {
  label: string
  position: [number, number]
}

/** Étiquettes des cercles 50, 100 et 150 km. */
export function distanceRingLabels(
  center: [number, number],
): DistanceRingLabel[] {
  const labels: DistanceRingLabel[] = []
  for (
    let radius = DISTANCE_RING_OPTIONS.majorEveryKm;
    radius <= DISTANCE_RING_OPTIONS.maxKm;
    radius += DISTANCE_RING_OPTIONS.majorEveryKm
  ) {
    labels.push({
      label: `${radius} km`,
      position: destinationPoint(center, 0, radius),
    })
  }
  return labels
}
