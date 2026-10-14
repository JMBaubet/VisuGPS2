//! Géométrie pure du module Communes.
//!
//! Aucune dépendance à Tauri, au disque ou à l'UI : distances, échantillonnage,
//! surfaces, appartenance et intersections, sur des coordonnées `[lon, lat]`
//! (degrés décimaux) — la convention GeoJSON reprise par le prototype.

use geo::{Coord, LineString, Simplify};

use super::osm::OsmPolygon;

/// Rayon terrestre moyen (m) — même constante que `import_gpx::haversine` et
/// `utils/geo.ts`.
const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// Deux sommets sont-ils le même nœud OSM ?
///
/// `out geom` répète les coordonnées **exactes** du nœud partagé entre deux ways
/// membres ; l'epsilon couvre les arrondis de sérialisation.
const NODE_EPSILON_DEG: f64 = 1e-9;

/// Même point, à l'epsilon des nœuds près.
pub fn same_point(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < NODE_EPSILON_DEG && (a[1] - b[1]).abs() < NODE_EPSILON_DEG
}

/// Distance géodésique 2D (Haversine) entre deux points `[lon, lat]`, en mètres.
pub fn haversine_m(a: [f64; 2], b: [f64; 2]) -> f64 {
    let phi1 = a[1].to_radians();
    let phi2 = b[1].to_radians();
    let half_dphi = ((phi2 - phi1) / 2.0).sin();
    let half_dlambda = ((b[0] - a[0]).to_radians() / 2.0).sin();
    let h = half_dphi * half_dphi + phi1.cos() * phi2.cos() * half_dlambda * half_dlambda;
    EARTH_RADIUS_M * 2.0 * h.sqrt().atan2((1.0 - h).sqrt())
}

/// Distances cumulées (mètres) le long d'une polyligne : `out[0] == 0`.
pub fn cumulative_distances(line: &[[f64; 2]]) -> Vec<f64> {
    let mut out = Vec::with_capacity(line.len());
    out.push(0.0);
    let mut total = 0.0;
    for w in line.windows(2) {
        total += haversine_m(w[0], w[1]);
        out.push(total);
    }
    out
}

/// Point situé à `d_m` mètres du départ, par interpolation **locale** entre les
/// deux sommets encadrants.
///
/// L'interpolation locale — et jamais un point le plus proche global — est ce
/// qui rend les kilomètres non ambigus sur une trace qui repasse au même
/// endroit : on sait toujours de quel passage on parle.
pub fn point_at_distance(line: &[[f64; 2]], cumul: &[f64], d_m: f64) -> [f64; 2] {
    if line.is_empty() {
        return [0.0, 0.0];
    }
    let last = line.len() - 1;
    if last == 0 || d_m <= 0.0 {
        return line[0];
    }
    if d_m >= cumul[last] {
        return line[last];
    }
    // Premier sommet dont la distance cumulée dépasse `d_m` ; le segment utile
    // est le précédent. `cumul[..last]` est croissant et `cumul[0] == 0 < d_m`,
    // donc l'indice obtenu est dans `1..=last`.
    let i = match cumul[..last].binary_search_by(|v| {
        v.partial_cmp(&d_m).unwrap_or(std::cmp::Ordering::Equal)
    }) {
        Ok(i) => i,
        Err(i) => i - 1,
    };
    let (a, b) = (line[i], line[i + 1]);
    let span = cumul[i + 1] - cumul[i];
    if span <= 0.0 {
        return a;
    }
    lerp(a, b, (d_m - cumul[i]) / span)
}

/// Bounding box `[minLon, minLat, maxLon, maxLat]` d'une polyligne.
pub fn bbox(line: &[[f64; 2]]) -> [f64; 4] {
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in line {
        b[0] = b[0].min(p[0]);
        b[1] = b[1].min(p[1]);
        b[2] = b[2].max(p[0]);
        b[3] = b[3].max(p[1]);
    }
    b
}

/// La bbox élargie contient-elle le point ?
pub fn bbox_contains(b: [f64; 4], p: [f64; 2]) -> bool {
    p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3]
}

/// Surface d'un anneau fermé (m²), formule sphérique du prototype
/// (`turf.area`). Nulle pour un anneau dégénéré.
pub fn ring_area_m2(ring: &[[f64; 2]]) -> f64 {
    if ring.len() < 4 {
        return 0.0;
    }
    let mut total = 0.0;
    for w in ring.windows(2) {
        total += (w[1][0] - w[0][0]).to_radians()
            * (2.0 + w[0][1].to_radians().sin() + w[1][1].to_radians().sin());
    }
    (total * EARTH_RADIUS_M * EARTH_RADIUS_M / 2.0).abs()
}

/// Surface **nette** d'un polygone : anneau extérieur moins ses trous (m²).
pub fn polygon_area_m2(poly: &OsmPolygon) -> f64 {
    let holes: f64 = poly.inners.iter().map(|r| ring_area_m2(r)).sum();
    (ring_area_m2(&poly.outer) - holes).max(0.0)
}

/// Centroïde d'un anneau : centroïde **pondéré par la surface** (formule de la
/// surface signée), avec repli sur la moyenne des sommets pour un anneau
/// dégénéré — comportement de `turf.centroid` sur un polygone.
pub fn ring_centroid(ring: &[[f64; 2]]) -> [f64; 2] {
    if ring.len() < 4 {
        return vertex_mean(ring);
    }
    let mut double_area = 0.0;
    let mut cx = 0.0;
    let mut cy = 0.0;
    for i in 0..ring.len() - 1 {
        let (x0, y0) = (ring[i][0], ring[i][1]);
        let (x1, y1) = (ring[i + 1][0], ring[i + 1][1]);
        let cross = x0 * y1 - x1 * y0;
        double_area += cross;
        cx += (x0 + x1) * cross;
        cy += (y0 + y1) * cross;
    }
    if double_area.abs() < f64::EPSILON {
        return vertex_mean(ring);
    }
    [cx / (3.0 * double_area), cy / (3.0 * double_area)]
}

/// Moyenne des sommets (un anneau fermé ne compte son premier sommet qu'une fois).
fn vertex_mean(ring: &[[f64; 2]]) -> [f64; 2] {
    let points = match ring.len() {
        0 => return [0.0, 0.0],
        n if same_point(ring[0], ring[n - 1]) => &ring[..n - 1],
        _ => ring,
    };
    let count = points.len().max(1) as f64;
    let sx: f64 = points.iter().map(|p| p[0]).sum();
    let sy: f64 = points.iter().map(|p| p[1]).sum();
    [sx / count, sy / count]
}

/// Centroïde de l'anneau extérieur d'un polygone.
pub fn polygon_centroid(poly: &OsmPolygon) -> [f64; 2] {
    ring_centroid(&poly.outer)
}

/// Le point est-il dans l'anneau ? (lancer de rayon)
pub fn point_in_ring(ring: &[[f64; 2]], p: [f64; 2]) -> bool {
    if ring.len() < 4 {
        return false;
    }
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let (xi, yi) = (ring[i][0], ring[i][1]);
        let (xj, yj) = (ring[j][0], ring[j][1]);
        if (yi > p[1]) != (yj > p[1]) {
            let crossing_x = (xj - xi) * (p[1] - yi) / (yj - yi) + xi;
            if p[0] < crossing_x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// Le point est-il dans le polygone : anneau extérieur, **hors** trous.
pub fn point_in_polygon(poly: &OsmPolygon, p: [f64; 2]) -> bool {
    point_in_ring(&poly.outer, p) && !poly.inners.iter().any(|hole| point_in_ring(hole, p))
}

/// Point du segment `[a, b]` au paramètre `t`.
pub fn lerp(a: [f64; 2], b: [f64; 2], t: f64) -> [f64; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// Intersection des segments `[a, b]` et `[c, d]`, exprimée par le paramètre
/// `t ∈ [0, 1]` de l'intersection le long de `[a, b]`.
///
/// Les segments parallèles ou colinéaires ne sont pas traités (aucune
/// intersection ponctuelle définie) : c'est le cas — rare et borné — d'une trace
/// qui **longe** exactement une frontière communale.
pub fn segment_intersection_t(
    a: [f64; 2],
    b: [f64; 2],
    c: [f64; 2],
    d: [f64; 2],
) -> Option<f64> {
    let r = [b[0] - a[0], b[1] - a[1]];
    let s = [d[0] - c[0], d[1] - c[1]];
    let denom = r[0] * s[1] - r[1] * s[0];
    if denom.abs() < 1e-15 {
        return None;
    }
    let qp = [c[0] - a[0], c[1] - a[1]];
    let t = (qp[0] * s[1] - qp[1] * s[0]) / denom;
    let u = (qp[0] * r[1] - qp[1] * r[0]) / denom;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some(t)
    } else {
        None
    }
}

/// Le segment `[a, b]` traverse-t-il la frontière de l'anneau ?
fn crosses_ring(ring: &[[f64; 2]], a: [f64; 2], b: [f64; 2]) -> bool {
    (0..ring.len().saturating_sub(1))
        .any(|i| segment_intersection_t(a, b, ring[i], ring[i + 1]).is_some())
}

/// La polyligne croise-t-elle le polygone ? (filtre préliminaire du prototype :
/// `booleanIntersects` — un sommet dedans, ou un franchissement de frontière)
pub fn line_intersects_polygon(line: &[[f64; 2]], poly: &OsmPolygon) -> bool {
    if line.iter().any(|p| point_in_polygon(poly, *p)) {
        return true;
    }
    line.windows(2).any(|w| {
        crosses_ring(&poly.outer, w[0], w[1])
            || poly.inners.iter().any(|hole| crosses_ring(hole, w[0], w[1]))
    })
}

/// Points d'intersection entre le segment `[a, b]` et la frontière du polygone
/// (anneau extérieur et trous), dans l'ordre d'énumération des arêtes.
pub fn boundary_crossings(poly: &OsmPolygon, a: [f64; 2], b: [f64; 2]) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    for ring in std::iter::once(&poly.outer).chain(poly.inners.iter()) {
        for i in 0..ring.len().saturating_sub(1) {
            if let Some(t) = segment_intersection_t(a, b, ring[i], ring[i + 1]) {
                out.push(lerp(a, b, t));
            }
        }
    }
    out
}

/// Simplification Douglas-Peucker à la tolérance donnée, **en degrés** — le
/// prototype (`turf.simplify`) opère lui aussi sur les coordonnées brutes.
pub fn simplify(line: &[[f64; 2]], tolerance: f64) -> Vec<[f64; 2]> {
    if line.len() < 3 || tolerance <= 0.0 {
        return line.to_vec();
    }
    let coords: Vec<Coord<f64>> = line
        .iter()
        .map(|p| Coord { x: p[0], y: p[1] })
        .collect();
    LineString::new(coords)
        .simplify(&tolerance)
        .0
        .into_iter()
        .map(|c| [c.x, c.y])
        .collect()
}
