//! Tests du contrat ORS algorithmique (`routing.rs`) : test d'identité des
//! tracés (IHM §7.3).

use crate::gpx_audit::geometry::Projector;
use crate::gpx_audit::routing::{project_from, routes_identical, ROUTE_SAME_TOL};
use crate::gpx_audit::types::AuditPoint;
use crate::gpx_audit::types::LatLon;

/// Tracé régulier vers l'est, `n` points échelonnés de `step_m` mètres.
fn line(n: usize, step_m: f64) -> Vec<LatLon> {
    let lat0 = 45.0;
    let m_per_deg_lon = (lat0 * std::f64::consts::PI / 180.0).cos() * 111320.0;
    (0..n)
        .map(|i| LatLon {
            lat: lat0,
            lon: 2.0 + (i as f64) * step_m / m_per_deg_lon,
        })
        .collect()
}

/// Tracé identique à `base` mais densifié (un point intermédiaire par segment).
fn densify(base: &[LatLon]) -> Vec<LatLon> {
    let mut out = Vec::new();
    for w in base.windows(2) {
        out.push(w[0].clone());
        out.push(LatLon {
            lat: (w[0].lat + w[1].lat) / 2.0,
            lon: (w[0].lon + w[1].lon) / 2.0,
        });
    }
    if let Some(last) = base.last() {
        out.push(last.clone());
    }
    out
}

/// Décale un tracé vers le nord de `d_m` mètres.
fn shift_north(route: &[LatLon], d_m: f64) -> Vec<LatLon> {
    let m_per_deg_lat = 110540.0;
    route
        .iter()
        .map(|p| LatLon {
            lat: p.lat + d_m / m_per_deg_lat,
            lon: p.lon,
        })
        .collect()
}

// ─── Projection ───────────────────────────────────────────────────────

/// Projecteur centré sur un point (lat, lon).
fn projector_at(lat: f64, lon: f64) -> Projector {
    Projector::new(&AuditPoint {
        id: 0,
        lat,
        lon,
        ele: None,
    })
}

#[test]
fn test_project_from_uses_given_origin() {
    let route = line(5, 100.0);
    let proj = projector_at(route[0].lat, route[0].lon);
    let pts = project_from(&proj, &route);
    assert_eq!(pts.len(), 5);
    assert!(pts[0].0.abs() < 1e-9, "x0 = {}", pts[0].0);
    assert!(pts[0].1.abs() < 1e-9, "y0 = {}", pts[0].1);
    // Pas de 100 m : la coordonnée x croît de 100 m par point.
    assert!((pts[1].0 - 100.0).abs() < 0.5, "x1 = {}", pts[1].0);
    assert!((pts[4].0 - 400.0).abs() < 2.0, "x4 = {}", pts[4].0);
}

#[test]
fn test_project_from_empty_route() {
    let proj = projector_at(45.0, 2.0);
    assert!(project_from(&proj, &[]).is_empty());
}

/// Contrat de projection : les **deux** tracés sont projetés dans un repère
/// commun. Un projecteur par tracé masquerait leur éloignement réel — c'est le
/// piège que ce test verrouille.
#[test]
fn test_two_routes_share_a_single_projection_origin() {
    let car = line(10, 100.0);
    let bike = shift_north(&car, 20.0);

    // Repère commun : le décalage reste mesurable.
    let shared = projector_at(car[0].lat, car[0].lon);
    let a = project_from(&shared, &car);
    let b = project_from(&shared, &bike);
    assert!((b[0].1 - a[0].1 - 20.0).abs() < 1e-6, "décalage perdu");

    // Repères distincts : le décalage disparaît (d'où l'exigence du repère
    // commun dans `routes_identical`).
    let self_proj = projector_at(bike[0].lat, bike[0].lon);
    let b_self = project_from(&self_proj, &bike);
    assert!((b_self[0].1 - a[0].1).abs() < 1e-6, "repères distincts");
}

// ─── Identité ─────────────────────────────────────────────────────────

#[test]
fn test_identical_when_routes_have_the_same_sampling() {
    let car = line(10, 100.0);
    let bike = line(10, 100.0);
    assert!(routes_identical(&car, 900.0, &bike, 900.0));
}

#[test]
fn test_identical_despite_sampling_density_difference() {
    // Le critère point→segment absorbe la densité d'échantillonnage : un tracé
    // densifié reste identique au tracé d'origine.
    let car = line(10, 100.0);
    let bike = densify(&car);
    assert!(routes_identical(&car, 900.0, &bike, 900.0));
    assert!(routes_identical(&bike, 900.0, &car, 900.0));
}

#[test]
fn test_different_when_one_route_deviates_beyond_tolerance() {
    let car = line(10, 100.0);
    // Écart de 20 m (> 15 m) : tracés différents.
    let bike = shift_north(&car, 20.0);
    assert!(!routes_identical(&car, 900.0, &bike, 900.0));
}

#[test]
fn test_identical_when_deviation_is_within_tolerance() {
    let car = line(10, 100.0);
    // Écart de 10 m (< 15 m) : toléré.
    let bike = shift_north(&car, 10.0);
    assert!(routes_identical(&car, 900.0, &bike, 900.0));
}

#[test]
fn test_different_when_lengths_differ_beyond_two_percent() {
    let car = line(10, 100.0);
    let bike = line(12, 100.0);
    // 1100 m contre 900 m : bien au-delà des 2 %.
    assert!(!routes_identical(&car, 900.0, &bike, 1100.0));
}

#[test]
fn test_length_tolerance_is_two_percent() {
    let car = line(10, 100.0);
    let bike = line(10, 100.0);
    // +1 % : accepté.
    assert!(routes_identical(&car, 1000.0, &bike, 1010.0));
    // +5 % : refusé.
    assert!(!routes_identical(&car, 1000.0, &bike, 1050.0));
}

#[test]
fn test_not_identical_when_one_route_is_missing() {
    let car = line(10, 100.0);
    assert!(!routes_identical(&car, 900.0, &[], 0.0));
    assert!(!routes_identical(&[], 0.0, &car, 900.0));
    assert!(!routes_identical(&[], 0.0, &[], 0.0));
}

#[test]
fn test_degenerate_single_point_routes() {
    // Deux tracés d'un seul point confondu : identiques (aucune distance).
    let a = vec![LatLon {
        lat: 45.0,
        lon: 2.0,
    }];
    assert!(routes_identical(&a, 10.0, &a, 10.0));
    // Un point décalé de plus de 15 m : différents.
    let b = shift_north(&a, 20.0);
    assert!(!routes_identical(&a, 10.0, &b, 10.0));
}

#[test]
fn test_same_length_but_different_spatial_span() {
    // Deux tracés de même longueur mais de forme différente : le critère
    // spatial les sépare.
    let car = line(10, 100.0);
    let mut zig: Vec<LatLon> = Vec::new();
    let lat0 = 45.0;
    let m_per_deg_lat = 110540.0;
    let m_per_deg_lon = (lat0 * std::f64::consts::PI / 180.0).cos() * 111320.0;
    for i in 0..10 {
        // Va-et-vient latéral : longueur comparable, emprise spatiale disjointe.
        let lateral = if i % 2 == 0 { 60.0 } else { -60.0 };
        zig.push(LatLon {
            lat: lat0 + lateral / m_per_deg_lat,
            lon: 2.0 + (i as f64) * 90.0 / m_per_deg_lon,
        });
    }
    assert!(!routes_identical(&car, 900.0, &zig, 900.0));
}

#[test]
fn test_tolerance_constant_is_fifteen_metres() {
    assert!((ROUTE_SAME_TOL - 15.0).abs() < 1e-9);
}
