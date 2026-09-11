//! Contrat d'échange avec OpenRouteService — partie algorithmique (IHM §7.3).
//!
//! Le HTML de référence exécute la comparaison des deux tracés en JavaScript,
//! où il dispose de sa projection locale. La géométrie étant du ressort du
//! portage Rust (décision 1), c'est ici que vit le test d'identité ; le réseau
//! lui-même reste côté front (`useAuditOrs.ts`).

use super::geometry::Projector;
use super::types::{AuditPoint, LatLon};

/// Tolérance de longueur relative entre les deux tracés (`ROUTE_SAME_LEN_TOL`).
pub const ROUTE_SAME_LEN_TOL: f64 = 0.02;

/// Tolérance de distance entre tracés (`ROUTE_SAME_TOL`), en mètres.
pub const ROUTE_SAME_TOL: f64 = 15.0;

/// Projecteur métrique local centré sur un point donné.
///
/// **Les deux tracés doivent partager le même projecteur** : la référence le
/// construit sur `car.coords[0]` et l'applique aux deux réponses. Deux
/// projecteurs distincts introduiraient un décalage d'origine entre les tracés,
/// qui rendrait identiques deux itinéraires pourtant éloignés.
fn projector_for(p: &LatLon) -> Projector {
    Projector::new(&AuditPoint {
        id: 0,
        lat: p.lat,
        lon: p.lon,
        ele: None,
    })
}

/// Projette un tracé dans le repère métrique d'un projecteur donné.
pub fn project_from(proj: &Projector, route: &[LatLon]) -> Vec<(f64, f64)> {
    route
        .iter()
        .map(|p| {
            proj.fwd(&AuditPoint {
                id: 0,
                lat: p.lat,
                lon: p.lon,
                ele: None,
            })
        })
        .collect()
}

/// Distance quadratique d'un point au segment `[a, b]`.
///
/// Le test point→**segment** (et non point→point) absorbe les différences de
/// densité d'échantillonnage entre les deux réponses ORS.
fn seg_dist2(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let abx = b.0 - a.0;
    let aby = b.1 - a.1;
    let l2 = abx * abx + aby * aby;
    let mut t = if l2 > 0.0 {
        ((p.0 - a.0) * abx + (p.1 - a.1) * aby) / l2
    } else {
        0.0
    };
    t = t.clamp(0.0, 1.0);
    let dx = p.0 - (a.0 + t * abx);
    let dy = p.1 - (a.1 + t * aby);
    dx * dx + dy * dy
}

/// Plus grande distance d'un tracé `x` à un tracé `y` (Hausdorff discrète,
/// sens `x → y`), en mètres.
fn max_dist(x: &[(f64, f64)], y: &[(f64, f64)]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    let mut m: f64 = 0.0;
    for &p in x {
        let mut best = f64::INFINITY;
        if y.len() < 2 {
            // Repli ponctuel (tracé d'un seul point, ou réponse dégénérée).
            if let Some(&only) = y.first() {
                let dx = p.0 - only.0;
                let dy = p.1 - only.1;
                best = dx * dx + dy * dy;
            }
        } else {
            for j in 0..y.len() - 1 {
                let d = seg_dist2(p, y[j], y[j + 1]);
                if d < best {
                    best = d;
                }
            }
        }
        if best.is_finite() && best > m {
            m = best;
        }
    }
    m.sqrt()
}

/// Deux tracés sont-ils identiques ? (portage de `routesIdentical`, IHM §7.3)
///
/// Deux critères, **tous deux requis** :
/// 1. longueurs totales à `ROUTE_SAME_LEN_TOL` près (relatif) ;
/// 2. distance de Hausdorff discrète point→segment, dans les **deux** sens,
///    inférieure ou égale à `ROUTE_SAME_TOL`.
///
/// Un tracé vide rend `false` : l'identité ne se déduit jamais d'une réponse
/// manquante.
pub fn routes_identical(
    car: &[LatLon],
    car_distance: f64,
    bike: &[LatLon],
    bike_distance: f64,
) -> bool {
    if car.is_empty() || bike.is_empty() {
        return false;
    }

    let lc = car_distance;
    let lb = bike_distance;
    if (lc - lb).abs() > ROUTE_SAME_LEN_TOL * lc.max(lb).max(1.0) {
        return false;
    }

    // Un seul projecteur pour les deux tracés (cf. `projector_for`).
    let proj = projector_for(&car[0]);
    let a = project_from(&proj, car);
    let b = project_from(&proj, bike);

    max_dist(&a, &b) <= ROUTE_SAME_TOL && max_dist(&b, &a) <= ROUTE_SAME_TOL
}
