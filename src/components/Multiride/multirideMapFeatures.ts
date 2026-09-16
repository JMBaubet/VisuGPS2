// Construction des features GeoJSON de la carte Multiride.
//
// Le fichier de description **ne stocke pas** la géométrie des emprunts : chaque
// emprunt y est borné à ses deux extrémités, et la portion complète se
// reconstitue en joignant `point_entree` / `point_sortie` avec la trace
// d'origine (annexe 13.6 de la spécification). C'est exactement ce que fait ce
// module, côté front : les bornes sont des numéros de points du GPX, donc des
// indices directs dans les points que le backend renvoie.
//
// Les fonctions sont pures : le composant de carte ne fait que pousser le
// résultat dans ses sources.

import type { MultiridePassage } from '../../stores/multiride'
import type { TracePoint } from '../../stores/traces'
import { sensLabel } from './multirideMapLayers'

/** Rendu complet de la vue : trace de fond, bornes et emprunts. */
export interface MultirideRender {
  /** Trace de fond (un seul trait, du premier au dernier point). */
  trace: GeoJSON.FeatureCollection
  /** Départ et arrivée de la trace. */
  ends: GeoJSON.FeatureCollection
  /** Un trait par emprunt, borné à la portion réellement parcourue. */
  passages: GeoJSON.FeatureCollection
  /** Emprise de la trace, pour le cadrage initial. */
  bounds: [number, number][] | null
}

/** Emprise d'une liste de points `[lon, lat]`. */
function boundsOf(coords: [number, number][]): [number, number][] | null {
  if (coords.length < 2) return null
  return coords
}

/**
 * Identifiant stable d'un emprunt, dans la source comme dans l'état de survol.
 * Chaîne : Mapbox accepte un identifiant numérique ou textuel.
 */
export function passageFeatureId(passage: MultiridePassage): string {
  return `s${passage.segment}-p${passage.passage}`
}

/**
 * HTML du popup d'un emprunt : numéro, sens, bornes de points et positions
 * kilométriques.
 */
function passagePopupHtml(passage: MultiridePassage): string {
  const ecarte = passage.fauxPositif ? ' · écarté' : ''
  return [
    '<div class="mrl-popup">',
    `<div><strong>Segment ${passage.segment} — emprunt ${passage.passage}</strong></div>`,
    `<div>${sensLabel(passage.sens)}${ecarte}</div>`,
    `<div>Points ${passage.pointEntree} → ${passage.pointSortie}</div>`,
    `<div>km ${passage.kmEntree.toFixed(2)} → ${passage.kmSortie.toFixed(2)}`,
    ` (${passage.longueurKm.toFixed(2)} km)</div>`,
    '</div>',
  ].join('')
}

/**
 * Découpe la portion d'un emprunt dans les points de la trace.
 *
 * `pointEntree` et `pointSortie` sont des numéros de points du GPX (1-based),
 * donc les indices `pointEntree - 1` à `pointSortie` inclus dans la liste des
 * points. Les bornes sont ramenées à la taille disponible : un fichier de
 * description plus ancien que le GPX ne doit pas faire échouer le rendu.
 */
export function passageCoords(
  passage: MultiridePassage,
  tracePoints: TracePoint[],
): [number, number][] {
  if (tracePoints.length < 2) return []
  const from = Math.min(Math.max(passage.pointEntree - 1, 0), tracePoints.length - 1)
  const to = Math.min(Math.max(passage.pointSortie, from + 1), tracePoints.length)
  return tracePoints.slice(from, to).map((p) => [p.lon, p.lat])
}

/** Feature d'un emprunt : sa portion complète, son sens et son popup. */
function passageFeature(
  passage: MultiridePassage,
  tracePoints: TracePoint[],
): GeoJSON.Feature | null {
  const coords = passageCoords(passage, tracePoints)
  if (coords.length < 2) return null
  return {
    type: 'Feature',
    id: passageFeatureId(passage),
    properties: {
      segment: passage.segment,
      passage: passage.passage,
      sens: passage.sens,
      fauxPositif: passage.fauxPositif,
      popupHtml: passagePopupHtml(passage),
    },
    geometry: { type: 'LineString', coordinates: coords },
  }
}

/**
 * Construit le rendu complet de la vue.
 *
 * La trace de fond et ses bornes sont émises même sans emprunt : la vue montre
 * toujours la trace sur laquelle la détection a porté.
 */
export function buildMultirideRender(
  passages: MultiridePassage[],
  tracePoints: TracePoint[],
): MultirideRender {
  const traceCoords: [number, number][] = tracePoints.map((p) => [p.lon, p.lat])

  const trace: GeoJSON.FeatureCollection =
    traceCoords.length < 2
      ? { type: 'FeatureCollection', features: [] }
      : {
          type: 'FeatureCollection',
          features: [
            {
              type: 'Feature',
              properties: { kind: 'trace' },
              geometry: { type: 'LineString', coordinates: traceCoords },
            },
          ],
        }

  const ends: GeoJSON.FeatureCollection =
    traceCoords.length < 2
      ? { type: 'FeatureCollection', features: [] }
      : {
          type: 'FeatureCollection',
          features: [
            {
              type: 'Feature',
              properties: { kind: 'start', popupHtml: '<div><strong>Départ</strong></div>' },
              geometry: { type: 'Point', coordinates: traceCoords[0] },
            },
            {
              type: 'Feature',
              properties: { kind: 'end', popupHtml: '<div><strong>Arrivée</strong></div>' },
              geometry: {
                type: 'Point',
                coordinates: traceCoords[traceCoords.length - 1],
              },
            },
          ],
        }

  const passageFeatures = passages
    .map((p) => passageFeature(p, tracePoints))
    .filter((f): f is GeoJSON.Feature => f !== null)

  return {
    trace,
    ends,
    passages: { type: 'FeatureCollection', features: passageFeatures },
    bounds: boundsOf(traceCoords),
  }
}

/**
 * Emprise des emprunts d'un segment, pour cadrer la carte sur son étendue.
 *
 * `null` si le segment n'a aucun emprunt rendable.
 */
export function segmentBounds(
  passages: MultiridePassage[],
  tracePoints: TracePoint[],
  segment: number,
): [number, number][] | null {
  const coords: [number, number][] = []
  for (const passage of passages) {
    if (passage.segment !== segment) continue
    coords.push(...passageCoords(passage, tracePoints))
  }
  return boundsOf(coords)
}
