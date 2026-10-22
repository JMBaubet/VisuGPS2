//! Géométrie pure du module Tunnels.
//!
//! Aucune dépendance à Tauri, au disque ou à l'UI. Les coordonnées suivent la
//! convention GeoJSON `[lon, lat]` (degrés décimaux), reprise par le prototype
//! et par `gpx_communes::geometry`.
//!
//! Le matching du prototype validé s'appuie sur :
//! - une projection locale équirectangulaire autour du barycentre de la trace ;
//! - un cap GPX calculé sur une fenêtre glissante (30 m par défaut) ;
//! - une distance point-à-segment et un écart angulaire bidirectionnel.

/// Rayon terrestre moyen (m) — même constante que `import_gpx::haversine`.
const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// Distance géodésique 2D (Haversine) entre deux points `[lon, lat]`, en mètres.
pub fn haversine_m(a: [f64; 2], b: [f64; 2]) -> f64 {
    let phi1 = a[1].to_radians();
    let phi2 = b[1].to_radians();
    let half_dphi = ((phi2 - phi1) / 2.0).sin();
    let half_dlambda = ((b[0] - a[0]).to_radians() / 2.0).sin();
    let h = half_dphi * half_dphi + phi1.cos() * phi2.cos() * half_dlambda * half_dlambda;
    EARTH_RADIUS_M * 2.0 * h.sqrt().atan2((1.0 - h).sqrt())
}

/// Cap initial de `from` vers `to`, en degrés (0 = nord, 90 = est).
pub fn bearing_deg(from: [f64; 2], to: [f64; 2]) -> f64 {
    let phi1 = from[1].to_radians();
    let phi2 = to[1].to_radians();
    let delta_lambda = (to[0] - from[0]).to_radians();
    let y = delta_lambda.sin() * phi2.cos();
    let x = phi1.cos() * phi2.sin() - phi1.sin() * phi2.cos() * delta_lambda.cos();
    (y.atan2(x).to_degrees() + 360.0) % 360.0
}

/// Distances cumulées (mètres) le long d'une polyligne : `out[0] == 0`.
pub fn cumulative_distances(line: &[[f64; 2]]) -> Vec<f64> {
    let mut out = Vec::with_capacity(line.len());
    if line.is_empty() {
        return out;
    }
    out.push(0.0);
    let mut total = 0.0;
    for window in line.windows(2) {
        total += haversine_m(window[0], window[1]);
        out.push(total);
    }
    out
}

/// Barycentre simple (moyenne des lon/lat) d'une polyligne ; `[0, 0]` si vide.
pub fn barycenter(line: &[[f64; 2]]) -> [f64; 2] {
    if line.is_empty() {
        return [0.0, 0.0];
    }
    let mut lon = 0.0;
    let mut lat = 0.0;
    for point in line {
        lon += point[0];
        lat += point[1];
    }
    let n = line.len() as f64;
    [lon / n, lat / n]
}

/// Bbox `[minLon, minLat, maxLon, maxLat]` d'une polyligne.
pub fn bbox(line: &[[f64; 2]]) -> [f64; 4] {
    if line.is_empty() {
        return [0.0, 0.0, 0.0, 0.0];
    }
    let mut out = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for point in line {
        out[0] = out[0].min(point[0]);
        out[1] = out[1].min(point[1]);
        out[2] = out[2].max(point[0]);
        out[3] = out[3].max(point[1]);
    }
    out
}

/// Bbox élargie d'une marge en mètres, `[minLon, minLat, maxLon, maxLat]`.
///
/// Approximation locale : 1° de latitude vaut ~111 km, 1° de longitude est
/// corrigé par le cosinus de la latitude médiane.
pub fn bbox_with_margin(line: &[[f64; 2]], margin_m: f64) -> [f64; 4] {
    let base = bbox(line);
    if line.is_empty() {
        return base;
    }
    let lat_mid = (base[1] + base[3]) / 2.0;
    let dlat = margin_m / 111_000.0;
    let dlon = margin_m / (111_000.0 * lat_mid.to_radians().cos().abs().max(1e-6));
    [
        base[0] - dlon,
        base[1] - dlat,
        base[2] + dlon,
        base[3] + dlat,
    ]
}

/// Deux bboxes se chevauchent-elles (bornes incluses) ?
pub fn bbox_intersects(a: [f64; 4], b: [f64; 4]) -> bool {
    !(a[2] < b[0] || a[0] > b[2] || a[3] < b[1] || a[1] > b[3])
}

/// La bbox contient-elle le point `[lon, lat]` (bornes incluses) ?
pub fn bbox_contains_point(bbox: [f64; 4], point: [f64; 2]) -> bool {
    point[0] >= bbox[0]
        && point[0] <= bbox[2]
        && point[1] >= bbox[1]
        && point[1] <= bbox[3]
}

/// Cap GPX au point `index`, calculé sur une fenêtre glissante de `window_m`
/// mètres de part et d'autre (algorithme du prototype validé).
pub fn cap_at(line: &[[f64; 2]], cumul: &[f64], index: usize, window_m: f64) -> f64 {
    if line.len() < 2 || cumul.len() != line.len() {
        return 0.0;
    }
    let i = index.min(line.len() - 1);
    let mut j = i;
    while j > 0 && (cumul[i] - cumul[j]) < window_m {
        j -= 1;
    }
    let mut k = i;
    while k < line.len() - 1 && (cumul[k] - cumul[i]) < window_m {
        k += 1;
    }
    if j == k {
        return 0.0;
    }
    bearing_deg(line[j], line[k])
}

/// Projection locale équirectangulaire autour d'un point de référence.
///
/// Retourne `(x, y)` en mètres : `x` vers l'est, `y` vers le nord. Valide à
/// quelques kilomètres ; le prototype découpe les traces plus longues.
pub fn project_local(point: [f64; 2], reference: [f64; 2]) -> (f64, f64) {
    let x = (point[0] - reference[0]).to_radians()
        * EARTH_RADIUS_M
        * reference[1].to_radians().cos();
    let y = (point[1] - reference[1]).to_radians() * EARTH_RADIUS_M;
    (x, y)
}

/// Distance point-à-segment en coordonnées projetées.
pub fn point_to_segment(
    px: f64,
    py: f64,
    ax: f64,
    ay: f64,
    bx: f64,
    by: f64,
) -> f64 {
    let dx = bx - ax;
    let dy = by - ay;
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return (px - ax).hypot(py - ay);
    }
    let t = (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0);
    let cx = ax + t * dx;
    let cy = ay + t * dy;
    (px - cx).hypot(py - cy)
}

/// Écart angulaire minimal entre deux caps, dans `[0, 180]`.
pub fn angle_diff_deg(a: f64, b: f64) -> f64 {
    let diff = (a - b).abs() % 360.0;
    if diff > 180.0 { 360.0 - diff } else { diff }
}

/// Cap d'un segment projeté, en degrés (0 = nord, 90 = est).
pub fn segment_cap_deg(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    ((bx - ax).atan2(by - ay).to_degrees() + 360.0) % 360.0
}
