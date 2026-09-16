//! Tests du module Multiride (passages multiples).

mod commands_test;
mod direction_test;
mod file_test;
mod projection_test;
mod resample_test;
mod runs_test;

use crate::gpx_multiride::projection::{build_geometry, MultirideGeom};
use crate::gpx_multiride::resample::{resample, Resample};
use crate::gpx_multiride::types::MultiridePoint;

/// Latitude du repère des traces de test.
const LAT0: f64 = 45.0;
/// Longitude du repère des traces de test.
const LON0: f64 = 2.0;
/// Mètres par degré de latitude — échelle de la projection locale.
const M_PER_DEG_LAT: f64 = 110_540.0;

/// Trace de test construite à partir de ses **sommets**, exprimés en mètres dans
/// le plan local (x vers l'est, y vers le nord), puis rééchantillonnée au pas
/// demandé.
///
/// Le premier sommet doit avoir `y = 0` : le repère de projection est centré sur
/// le premier point, et la correspondance mètres → degrés employée ici est
/// exactement celle de la projection dans ce cas. Les sommets sont reliés par des
/// segments droits, ce qui rend les intervalles attendus calculables à la main —
/// les tests peuvent donc affirmer des valeurs exactes plutôt que des ordres de
/// grandeur.
fn sampled_trace(waypoints: &[(f64, f64)], step_m: f64) -> (MultirideGeom, Resample) {
    let kx = LAT0.to_radians().cos() * 111_320.0;
    let points: Vec<MultiridePoint> = waypoints
        .iter()
        .map(|(x, y)| MultiridePoint {
            lat: LAT0 + y / M_PER_DEG_LAT,
            lon: LON0 + x / kx,
        })
        .collect();

    let geom = build_geometry(&points).expect("géométrie de test");
    let sampling = resample(&geom, step_m).expect("rééchantillonnage de test");
    (geom, sampling)
}
