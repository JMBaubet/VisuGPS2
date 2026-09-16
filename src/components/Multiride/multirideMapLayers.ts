// Contrat de rendu de la vue Multiride : palette, épaisseurs et couches Mapbox.
//
// Ce fichier est partagé par la **carte** et par le **panneau des segments** :
// la couleur d'un emprunt ne dit pas seulement quelque chose sur la carte, elle
// identifie le même emprunt dans le ruban et la liste. La palette est donc
// définie ici une fois pour les deux.
//
// **Palette Material Design**, y compris les trois couleurs de la spécification
// (bleu `#3564e0`, orange `#ec9c11`, rouge `#dd3327`), traduites en nuances 500 :
// la couleur d'un emprunt doit rester celle du reste de l'application, où
// l'orange signifie « parcouru » et le rouge « écarté ». Voir la note de
// portage : la bibliothèque Material n'a pas de noir absolu, l'arrivée est donc
// en `grey 900`.
//
// Mapbox GL n'a pas de « panes » nommés : l'empilement est déterminé par
// l'**ordre d'ajout** des couches (bas → haut). C'est ce qui impose **trois
// couches** pour les emprunts — une par sens — plutôt qu'une couche unique aux
// couleurs pilotées par les données : la référence doit rester **sous** les
// emprunts qui la recouvrent, et Mapbox ne sait ordonner qu'entre couches.

import type {
  CircleLayerSpecification,
  ExpressionSpecification,
  FilterSpecification,
  GeoJSONSource,
  LineLayerSpecification,
  Map,
  MapLayerMouseEvent,
} from 'mapbox-gl'
import type { MultirideSens } from '../../stores/multiride'

/** Collection de features vide (source neutre avant tout rendu). */
export function emptyFC(): GeoJSON.FeatureCollection {
  return { type: 'FeatureCollection', features: [] }
}

/** Palette des emprunts et de la trace de fond. */
export const MULTIRIDE_COLORS = {
  /** Trace de fond — `grey 500`, neutre et en retrait. */
  trace: '#9E9E9E',
  /** Emprunt de référence — `blue 500`, le bleu primaire de l'application. */
  reference: '#2196F3',
  /** Emprunt de même sens — `orange 500`. */
  aller: '#FF9800',
  /** Emprunt en sens inverse — `red 500`. */
  retour: '#F44336',
  /** Départ — blanc. */
  start: '#FFFFFF',
  /** Arrivée — `grey 900` (la palette Material n'a pas de noir absolu). */
  end: '#212121',
} as const

/**
 * Épaisseurs et opacités de la spécification (§F-11) : la référence est le
 * trait le plus épais, et l'épaisseur décroissante distingue les sens même sur
 * une capture sans légende.
 */
export const MULTIRIDE_STYLE = {
  traceWidth: 2,
  referenceWidth: 8,
  allerWidth: 6,
  retourWidth: 4,
  referenceOpacity: 0.85,
  passageOpacity: 0.95,
  /** Opacité d'un segment écarté par l'utilisateur (faux positif). */
  falsePositiveOpacity: 0.15,
} as const

/** Sources de la carte. */
export const SOURCE_IDS = {
  trace: 'mrl-trace',
  passages: 'mrl-passages',
} as const

/** Identifiants des couches, dans l'ordre d'empilement (bas → haut). */
export const LAYER_IDS = {
  trace: 'mrl-trace-line',
  reference: 'mrl-passage-reference',
  aller: 'mrl-passage-aller',
  retour: 'mrl-passage-retour',
  traceEnds: 'mrl-trace-ends',
} as const

/** Les trois couches d'emprunts, dans leur ordre d'empilement. */
export const PASSAGE_LAYER_IDS: string[] = [
  LAYER_IDS.reference,
  LAYER_IDS.aller,
  LAYER_IDS.retour,
]

/** Style de carte et vue de repli (comme les autres cartes de l'application). */
export const MULTIRIDE_MAP_STYLE = 'mapbox://styles/mapbox/outdoors-v12'
export const MULTIRIDE_MAP_CENTER: [number, number] = [2.6, 46.6]
export const MULTIRIDE_MAP_ZOOM = 6

/** Couleur d'un emprunt selon son sens. */
export function sensColor(sens: MultirideSens): string {
  if (sens === 'reference') return MULTIRIDE_COLORS.reference
  return sens === 'aller' ? MULTIRIDE_COLORS.aller : MULTIRIDE_COLORS.retour
}

/** Libellé utilisateur d'un sens. */
export function sensLabel(sens: MultirideSens): string {
  if (sens === 'reference') return 'Référence'
  return sens === 'aller' ? 'Aller' : 'Retour'
}

/** Épaisseur de trait d'un emprunt selon son sens. */
export function sensWidth(sens: MultirideSens): number {
  if (sens === 'reference') return MULTIRIDE_STYLE.referenceWidth
  return sens === 'aller' ? MULTIRIDE_STYLE.allerWidth : MULTIRIDE_STYLE.retourWidth
}

/**
 * Filtre d'une couche d'emprunts : une couche par sens, pour que la référence
 * reste sous les emprunts qui la recouvrent.
 */
function sensFilter(sens: MultirideSens): FilterSpecification {
  return ['==', ['get', 'sens'], sens] as unknown as FilterSpecification
}

/**
 * Opacité de la couche : réduite pour un emprunt appartenant à un segment que
 * l'utilisateur a écarté.
 */
function opacityExpression(defaultOpacity: number): ExpressionSpecification {
  return [
    'case',
    ['boolean', ['get', 'fauxPositif'], false],
    MULTIRIDE_STYLE.falsePositiveOpacity,
    defaultOpacity,
  ] as unknown as ExpressionSpecification
}

/**
 * Épaisseur réactive de la couche : le survol épaissit l'emprunt pointé
 * (spécification §F-13), via l'état de feature — la donnée n'est pas retouchée.
 */
function widthExpression(sens: MultirideSens): ExpressionSpecification {
  const width = sensWidth(sens)
  return [
    'case',
    ['boolean', ['feature-state', 'hover'], false],
    width + 3,
    width,
  ] as unknown as ExpressionSpecification
}

/** Traits ronds aux extrémités : une trace n'a pas d'angle vif. */
const LINE_LAYOUT = { 'line-cap': 'round', 'line-join': 'round' } as const

/** Poignées d'interaction posées sur les couches d'emprunts. */
export interface MultirideMapHandlers {
  /** Clic sur un emprunt : popup et sélection du segment. */
  onClick: (e: MapLayerMouseEvent) => void
  /** Entrée du pointeur : épaississement. */
  onEnter: (e: MapLayerMouseEvent) => void
  /** Sortie du pointeur : retour au trait nominal. */
  onLeave: (e: MapLayerMouseEvent) => void
}

/**
 * Crée les sources et les couches de la carte, dans l'ordre d'empilement :
 * trace de fond, puis référence, aller et retour, puis les bornes de la trace.
 *
 * Idempotente : un second appel après un `load` sans destruction préalable est
 * sans effet.
 */
export function initMultirideLayers(map: Map, handlers: MultirideMapHandlers): void {
  if (map.getSource(SOURCE_IDS.trace)) return

  map.addSource(SOURCE_IDS.trace, { type: 'geojson', data: emptyFC() })
  map.addSource(SOURCE_IDS.passages, { type: 'geojson', data: emptyFC() })

  const traceLayer: LineLayerSpecification = {
    id: LAYER_IDS.trace,
    type: 'line',
    source: SOURCE_IDS.trace,
    filter: ['==', ['get', 'kind'], 'trace'] as unknown as FilterSpecification,
    layout: LINE_LAYOUT,
    paint: {
      'line-color': MULTIRIDE_COLORS.trace,
      'line-width': MULTIRIDE_STYLE.traceWidth,
      'line-opacity': 0.9,
    },
  }
  map.addLayer(traceLayer)

  // Les emprunts, du plus épais au plus fin : la référence passe **sous** les
  // emprunts qui la recouvrent.
  for (const layerId of PASSAGE_LAYER_IDS) {
    const sens: MultirideSens =
      layerId === LAYER_IDS.reference
        ? 'reference'
        : layerId === LAYER_IDS.aller
          ? 'aller'
          : 'retour'
    const layer: LineLayerSpecification = {
      id: layerId,
      type: 'line',
      source: SOURCE_IDS.passages,
      filter: sensFilter(sens),
      layout: LINE_LAYOUT,
      paint: {
        'line-color': sensColor(sens),
        'line-width': widthExpression(sens),
        'line-opacity': opacityExpression(
          sens === 'reference'
            ? MULTIRIDE_STYLE.referenceOpacity
            : MULTIRIDE_STYLE.passageOpacity,
        ),
      },
    }
    map.addLayer(layer)
  }

  // Bornes de la trace : dessus, elles ne doivent pas être recouvertes. Aucun
  // filtre : la source mêle la trace et ses deux bornes, et le type de géométrie
  // suffit à ne dessiner que les points.
  const endsLayer: CircleLayerSpecification = {
    id: LAYER_IDS.traceEnds,
    type: 'circle',
    source: SOURCE_IDS.trace,
    paint: {
      'circle-radius': 6,
      'circle-color': [
        'case',
        ['==', ['get', 'kind'], 'start'],
        MULTIRIDE_COLORS.start,
        MULTIRIDE_COLORS.end,
      ] as unknown as ExpressionSpecification,
      'circle-stroke-color': '#FFFFFF',
      'circle-stroke-width': 2,
    },
  }
  map.addLayer(endsLayer)

  for (const layerId of PASSAGE_LAYER_IDS) {
    map.on('click', layerId, handlers.onClick)
    map.on('mouseenter', layerId, handlers.onEnter)
    map.on('mouseleave', layerId, handlers.onLeave)
  }
}

/** Met à jour la trace de fond et ses bornes. */
export function setTraceData(
  map: Map,
  trace: GeoJSON.FeatureCollection,
  ends: GeoJSON.FeatureCollection,
): void {
  const source = map.getSource(SOURCE_IDS.trace) as GeoJSONSource | undefined
  if (!source) return
  // Une seule source porte la trace et ses bornes : les deux rendus sont
  // fusionnés, chaque feature portant son `kind`.
  source.setData({
    type: 'FeatureCollection',
    features: [...trace.features, ...ends.features],
  })
}

/** Met à jour les emprunts rendus. */
export function setPassageData(map: Map, passages: GeoJSON.FeatureCollection): void {
  const source = map.getSource(SOURCE_IDS.passages) as GeoJSONSource | undefined
  if (source) source.setData(passages)
}
