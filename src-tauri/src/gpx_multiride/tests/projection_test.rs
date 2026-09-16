//! Tests de `gpx_multiride::projection` — projection locale et géométrie
//! métrique.
//!
//! Ils fixent deux choses dont dépend tout le reste du module : l'**échelle** de
//! la projection (les seuils de détection sont exprimés en mètres, la conversion
//! doit donc être juste) et la **provenance** de chaque point conservé, sans
//! laquelle les numéros de points du fichier de description ne désigneraient pas
//! les bons `<trkpt>`.

use crate::gpx_multiride::projection::{build_geometry, Projector, DUP_THRESHOLD2};
use crate::gpx_multiride::types::MultiridePoint;

// ─── Aides de test ────────────────────────────────────────────────────

/// Mètres par degré de latitude — échelle `ky` de la projection locale.
const M_PER_DEG_LAT: f64 = 110_540.0;
/// Mètres par degré de longitude à l'équateur, avant compensation en `cos(lat₀)`.
const M_PER_DEG_LON: f64 = 111_320.0;

fn point(lat: f64, lon: f64) -> MultiridePoint {
    MultiridePoint { lat, lon }
}

/// Trace rectiligne vers le nord : `count` points espacés de `spacing_m`.
fn line_north(count: usize, spacing_m: f64) -> Vec<MultiridePoint> {
    let dlat = spacing_m / M_PER_DEG_LAT;
    (0..count)
        .map(|i| point(45.0 + (i as f64) * dlat, 2.0))
        .collect()
}

/// Écart maximal toléré entre deux flottants calculés par deux chemins.
const EPS: f64 = 1e-9;

// ─── Projection ───────────────────────────────────────────────────────

/// L'échelle de la projection est celle de la spécification : 110 540 m par
/// degré de latitude, `111 320 · cos(lat₀)` par degré de longitude.
#[test]
fn projection_scales_follow_degrees() {
    let proj = Projector::new(&point(45.0, 2.0));

    let (x, y) = proj.fwd(&point(46.0, 3.0));

    assert!((y - M_PER_DEG_LAT).abs() < EPS, "y = {y}");
    let expected_x = M_PER_DEG_LON * (45.0_f64 * std::f64::consts::PI / 180.0).cos();
    assert!((x - expected_x).abs() < EPS, "x = {x}");
}

/// `inv ∘ fwd = id` : la projection est réversible, ce dont dépend la
/// reconstruction des coordonnées des points échantillonnés.
#[test]
fn projection_round_trips() {
    let proj = Projector::new(&point(45.0, 2.0));
    let p = point(45.1234, 2.5678);

    let (x, y) = proj.fwd(&p);
    let (lat, lon) = proj.inv(x, y);

    assert!((lat - p.lat).abs() < EPS);
    assert!((lon - p.lon).abs() < EPS);
}

/// Le repère est centré sur le premier point : il s'y projette en (0, 0).
#[test]
fn projection_is_centred_on_the_first_point() {
    let proj = Projector::new(&point(45.0, 2.0));
    let (x, y) = proj.fwd(&point(45.0, 2.0));
    assert_eq!((x, y), (0.0, 0.0));
}

// ─── Géométrie ────────────────────────────────────────────────────────

/// Les distances cumulées sont métriques : quatre intervalles de 250 m font
/// 1 000 m, et la distance est monotone.
#[test]
fn build_geometry_measures_distances_in_meters() {
    let geom = build_geometry(&line_north(5, 250.0)).unwrap();

    assert_eq!(geom.px.len(), 5);
    assert_eq!(geom.cum[0], 0.0);
    assert!((geom.total - 1000.0).abs() < EPS, "total = {}", geom.total);
    for i in 1..geom.cum.len() {
        assert!(geom.cum[i] > geom.cum[i - 1], "cum non monotone en {i}");
    }
}

/// Un point à moins de 5 cm du dernier point conservé est écarté, et le point
/// conservé porte l'index de sa **première** occurrence.
#[test]
fn build_geometry_drops_duplicates_and_keeps_provenance() {
    let d = |meters: f64| meters / M_PER_DEG_LAT;
    let points = vec![
        point(45.0, 2.0),              // 0 — conservé
        point(45.0 + d(0.01), 2.0),    // 1 — à 1 cm : écarté
        point(45.0 + d(0.04), 2.0),    // 2 — à 4 cm : écarté
        point(45.0 + d(0.10), 2.0),    // 3 — à 10 cm : conservé
        point(45.0 + d(100.0), 2.0),   // 4 — conservé
    ];

    let geom = build_geometry(&points).unwrap();

    assert_eq!(geom.px.len(), 3, "trois points distincts attendus");
    assert_eq!(geom.raw_index, vec![0, 3, 4]);
    assert!((geom.total - 100.0).abs() < 1e-6, "total = {}", geom.total);
}

/// Le seuil de dédoublonnage est celui de la spécification : (0,05 m)². Le
/// modifier changerait silencieusement le comportement de la détection.
#[test]
fn duplicate_threshold_is_five_centimeters() {
    assert_eq!(DUP_THRESHOLD2, 0.0025);
}

/// Une trace dégénérée est refusée, avec un message qui distingue l'absence de
/// points de l'absence de points **distincts**.
#[test]
fn build_geometry_rejects_degenerate_traces() {
    let empty = build_geometry(&[]).unwrap_err();
    assert!(empty.contains("moins de 2 points"), "message : {empty}");

    let single = build_geometry(&line_north(1, 100.0)).unwrap_err();
    assert!(single.contains("moins de 2 points"), "message : {single}");

    let coincident = vec![point(45.0, 2.0), point(45.0, 2.0), point(45.0, 2.0)];
    let error = build_geometry(&coincident).unwrap_err();
    assert!(error.contains("points distincts"), "message : {error}");
}
