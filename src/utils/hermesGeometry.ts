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
