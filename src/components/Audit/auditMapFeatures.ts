// Construction des features GeoJSON d'une anomalie (IHM §5.1).
//
// Module **sans dépendance UI** : il ne manipule que des données (points,
// finding, éléments de rendu) et retourne des features neutres. Le composant se
// charge de les publier dans les registres de couches.
//
// Les géométries sont hétérogènes dans un layer unique : chaque feature porte
// ses propriétés visuelles (`color`, `width`, `opacity`, `radius`, `fill`,
// `stroke`, `strokeWidth`) et son `popupHtml`, conformément au rendu
// data-driven de la référence.

import type { AuditPoint, Finding, MapOverlay } from '../../stores/audit'
import { AUDIT_COLORS } from './auditMapLayers'

/** Propriétés visuelles et popup portées par une feature. */
type FeatureProps = Record<string, unknown>

/** Nombre de points agrégés par la feature (indices de trace, base 0). */
const CTX_TEXT_UP = 'amont'
const CTX_TEXT_DN = 'aval'

/** Numéro de point affiché (base 1) — `ptNo` du JS. */
function ptNo(i: number): number {
  return i + 1
}

/** Coordonnée `[lon, lat]` du point d'indice `i`. */
function lngLat(points: AuditPoint[], i: number): [number, number] {
  const p = points[i]
  return [p?.lon ?? Number.NaN, p?.lat ?? Number.NaN]
}

/** Popup minimal : titre en gras puis lignes de détail. */
function popupHtml(title: string, rows: string[]): string {
  const head = `<div><strong>${title}</strong></div>`
  const body = rows.map((r) => `<div>${r}</div>`).join('')
  return head + body
}

/** Coordonnées affichées à 5 décimales (`lat, lon`). */
function coordText(lat: number, lon: number): string {
  return `${lat.toFixed(5)}, ${lon.toFixed(5)}`
}

/** Feature `Point` avec ses propriétés visuelles. */
function pointFeature(
  coords: [number, number],
  props: FeatureProps,
  idx?: number,
): GeoJSON.Feature {
  const f: GeoJSON.Feature = {
    type: 'Feature',
    properties: props,
    geometry: { type: 'Point', coordinates: coords },
  }
  if (idx !== undefined) f.properties = { ...props, _idx: idx }
  return f
}

/** Feature `LineString` avec ses propriétés visuelles. */
function lineFeature(
  coords: [number, number][],
  props: FeatureProps,
): GeoJSON.Feature {
  return {
    type: 'Feature',
    properties: props,
    geometry: { type: 'LineString', coordinates: coords },
  }
}

/** Résultat de rendu d'une anomalie. */
export interface FindingRender {
  /** Traits à publier dans `lyr-anomaly-lines`. */
  lines: GeoJSON.Feature[]
  /** Points à publier dans `lyr-anomaly-points`. */
  points: GeoJSON.Feature[]
  /** Tracé de routage appliqué (anomalie corrigée), sinon collection vide. */
  applied: GeoJSON.FeatureCollection
  /**
   * Coordonnées couvertes par le rendu : le composant en déduit les bornes de
   * centrage (`fitBounds`) sans reconstruire la géométrie.
   */
  bounds: [number, number][]
}

/**
 * Construit le rendu d'une anomalie selon son type et son statut.
 *
 * `overlay` fournit les ancres de routage RP (couleur et rayon des points
 * d'ancre) ; il peut être `null` tant que la commande de rendu n'a pas répondu.
 */
export function buildFindingRender(
  finding: Finding,
  points: AuditPoint[],
  overlay: MapOverlay | null,
): FindingRender {
  const lines: GeoJSON.Feature[] = []
  const out: GeoJSON.Feature[] = []
  const bounds: [number, number][] = []

  // ─── Anomalie corrigée : points d'origine en gris ──────────────────
  if (finding.status === 'corrected') {
    const undo = finding.undo
    // Seule une correction par routage porte un tracé et ses extrémités.
    const routeUndo = undo && undo.type === 'route' ? undo : null
    const orig = undo ? undo.origPts : []
    let applied: GeoJSON.FeatureCollection = {
      type: 'FeatureCollection',
      features: [],
    }

    // Tracé de routage appliqué (sous la trace).
    if (finding.correction?.startsWith('route') && routeUndo?.routePts?.length) {
      const coords: [number, number][] = routeUndo.routePts.map((p) => [
        p.lon,
        p.lat,
      ])
      const kind =
        finding.correction === 'route-car' ? 'voiture' : 'vélo de route'
      applied = {
        type: 'FeatureCollection',
        features: [
          lineFeature(coords, {
            popupHtml: popupHtml(finding.label, [
              `Tracé de routage appliqué (${kind})`,
              `points insérés : <strong>${coords.length}</strong>`,
            ]),
          }),
        ],
      }
    }

    orig.forEach((p, rank) => {
      const coords: [number, number] = [p.lon, p.lat]
      out.push(
        pointFeature(coords, {
          radius: 3.5,
          fill: AUDIT_COLORS.orig,
          stroke: '#ffffff',
          strokeWidth: 1.2,
          popupHtml: popupHtml(finding.label, [
            `Point d'origine · ex-pt <strong>${(undo?.firstNo ?? 1) + rank}</strong>`,
          ]),
        }),
      )
      bounds.push(coords)
    })

    // Bornes de la correction : extrémités conservées du routage appliqué.
    if (finding.correction?.startsWith('route')) {
      const bounds2: Array<{ pt: { lat: number; lon: number } | undefined; color: string; text: string }> = [
        {
          pt: routeUndo?.startPt,
          color: AUDIT_COLORS.selA,
          text: 'Point Début de la correction (conservé)',
        },
        {
          pt: routeUndo?.endPt,
          color: AUDIT_COLORS.selB,
          text: 'Point Fin de la correction (conservé)',
        },
      ]
      for (const b of bounds2) {
        if (!b.pt) continue
        const coords: [number, number] = [b.pt.lon, b.pt.lat]
        out.push(
          pointFeature(coords, {
            radius: 3.5,
            fill: b.color,
            stroke: '#ffffff',
            strokeWidth: 1.2,
            popupHtml: popupHtml(finding.label, [
              `${b.text} · lat/lon : <strong>${coordText(b.pt.lat, b.pt.lon)}</strong>`,
            ]),
          }),
        )
        bounds.push(coords)
      }
    }

    return { lines, points: out, applied, bounds }
  }

  const isFp = finding.status === 'fp'
  const col = isFp ? AUDIT_COLORS.fp : AUDIT_COLORS.warn
  const part0 = finding.parts[0]
  if (!part0) return { lines, points: out, applied: { type: 'FeatureCollection', features: [] }, bounds }
  const s = part0.s
  const e = part0.e

  // ─── Boucle giratoire : zone, cœur, points et ancres ───────────────
  if (finding.kind === 'rp') {
    const part1 = finding.parts[1]
    const cs = part1?.s ?? s
    const ce = part1?.e ?? e

    const zone: [number, number][] = []
    for (let i = s; i <= e; i++) zone.push(lngLat(points, i))
    if (zone.length >= 2) {
      lines.push(
        lineFeature(zone, {
          color: col,
          width: 2.5,
          opacity: 0.8,
          popupHtml: popupHtml(finding.label, [
            finding.summary,
            `Emprise : <strong>${ptNo(s)} → ${ptNo(e)}</strong>`,
          ]),
        }),
      )
    }

    const core: [number, number][] = []
    for (let i = cs; i <= ce; i++) core.push(lngLat(points, i))
    if (core.length >= 2) {
      lines.push(
        lineFeature(core, {
          color: col,
          width: 5,
          opacity: 0.95,
          popupHtml: popupHtml(finding.label, [
            part1?.text ?? '',
            `Cœur : <strong>${ptNo(cs)} → ${ptNo(ce)}</strong>`,
          ]),
        }),
      )
    }
    bounds.push(...zone)

    const up = overlay?.anchors?.up ?? null
    const dn = overlay?.anchors?.dn ?? null
    for (let i = s; i <= e; i++) {
      const isAnchor = i === up || i === dn
      const coords = lngLat(points, i)
      const lat = points[i]?.lat ?? Number.NaN
      const lon = points[i]?.lon ?? Number.NaN
      const fill = isAnchor
        ? i === up
          ? AUDIT_COLORS.anchorA
          : AUDIT_COLORS.anchorB
        : col

      let html: string
      if (isAnchor) {
        const role =
          i === up
            ? 'ancre de routage amont — début de la zone remplacée par défaut'
            : 'ancre de routage aval — fin de la zone remplacée par défaut'
        html = popupHtml(finding.label, [
          `${role} · pt <strong>${ptNo(i)}</strong>`,
          `lat/lon : <strong>${coordText(lat, lon)}</strong>`,
        ])
      } else {
        html = popupHtml(finding.label, [
          `pt <strong>${ptNo(i)}</strong>`,
          `lat/lon : <strong>${coordText(lat, lon)}</strong>`,
        ])
      }

      out.push(
        pointFeature(
          coords,
          {
            radius: isAnchor ? 7 : 2.5,
            fill,
            stroke: '#ffffff',
            strokeWidth: isAnchor ? 2 : 1,
            popupHtml: html,
          },
          i,
        ),
      )
    }
  } else {
    // ─── Aller-retour : emprise et points ────────────────────────────
    const coords: [number, number][] = []
    for (let i = s; i <= e; i++) coords.push(lngLat(points, i))
    if (coords.length >= 2) {
      lines.push(
        lineFeature(coords, {
          color: col,
          width: 5,
          opacity: 0.95,
          popupHtml: popupHtml(finding.label, [
            finding.summary,
            `Points : <strong>${ptNo(s)} → ${ptNo(e)}</strong>`,
          ]),
        }),
      )
    }
    bounds.push(...coords)

    for (let i = s; i <= e; i++) {
      const lat = points[i]?.lat ?? Number.NaN
      const lon = points[i]?.lon ?? Number.NaN
      out.push(
        pointFeature(
          lngLat(points, i),
          {
            radius: 4,
            fill: col,
            stroke: '#ffffff',
            strokeWidth: 1.5,
            popupHtml: popupHtml(finding.label, [
              `pt <strong>${ptNo(i)}</strong>`,
              `lat/lon : <strong>${coordText(lat, lon)}</strong>`,
            ]),
          },
          i,
        ),
      )
    }
  }

  // ─── Points de contexte sains (jamais sur un faux positif) ─────────
  if (!isFp) {
    for (const side of ['up', 'dn'] as const) {
      const ci = finding.ctx[side]
      if (ci === null || ci === undefined) continue
      const coords = lngLat(points, ci)
      out.push(
        pointFeature(coords, {
          radius: 5,
          fill: AUDIT_COLORS.ctx,
          stroke: '#ffffff',
          strokeWidth: 1.5,
          popupHtml: popupHtml(finding.label, [
            `pt <strong>${ptNo(ci)}</strong> · ${
              side === 'up' ? CTX_TEXT_UP : CTX_TEXT_DN
            } (sain)`,
          ]),
        }),
      )
      bounds.push(coords)
    }
  }

  return { lines, points: out, applied: { type: 'FeatureCollection', features: [] }, bounds }
}
