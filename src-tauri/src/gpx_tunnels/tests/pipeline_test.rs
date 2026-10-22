//! Tests du pipeline de matching.

use std::collections::BTreeMap;
use std::path::Path;

use crate::gpx_tunnels::pbf::OsmWay;
use crate::gpx_tunnels::pipeline::{build_file, load_points, match_tunnels};
use crate::gpx_tunnels::types::{PbfFileRef, TunnelParams, FILE_VERSION};

fn way(id: &str, coords: Vec<[f64; 2]>) -> OsmWay {
    let mut tags = BTreeMap::new();
    tags.insert("highway".to_string(), "primary".to_string());
    tags.insert("tunnel".to_string(), "yes".to_string());
    tags.insert("name".to_string(), "Tunnel de test".to_string());
    OsmWay {
        id: id.to_string(),
        tags,
        coords,
    }
}

fn east_way(id: &str, lat: f64, start_lon: f64, end_lon: f64) -> OsmWay {
    way(id, vec![[start_lon, lat], [end_lon, lat]])
}

fn trace(start_lon: f64, end_lon: f64, lat: f64, step: f64) -> Vec<[f64; 2]> {
    let count = ((end_lon - start_lon) / step).round() as usize;
    (0..=count)
        .map(|index| [start_lon + index as f64 * step, lat])
        .collect()
}

#[test]
fn matches_a_straight_tunnel() {
    let way = east_way("w1", 45.0, 2.0, 2.003);
    let points = trace(1.999, 2.004, 45.00002, 0.0001);

    let tunnels = match_tunnels(&points, &[way], &TunnelParams::default());

    assert_eq!(tunnels.len(), 1);
    let tunnel = &tunnels[0];
    assert_eq!(tunnel.way_id.as_deref(), Some("w1"));
    assert_eq!(tunnel.name.as_deref(), Some("Tunnel de test"));
    assert_eq!(tunnel.highway.as_deref(), Some("primary"));
    assert_eq!(tunnel.tunnel_type.as_deref(), Some("yes"));
    assert_eq!(tunnel.detection_method, "osm");
    assert!(tunnel.group_id.is_none());
    assert!(tunnel.start_km > 0.0);
    assert!(tunnel.end_km > tunnel.start_km);
    assert!(tunnel.length_m > 100.0);
    assert!(tunnel.max_distance_m.unwrap() <= 20.0 + 1e-6);
    assert!(tunnel.max_angle_diff_deg.unwrap() <= 25.0 + 1e-6);
}

#[test]
fn rejects_a_perpendicular_way() {
    let way = way("w1", vec![[2.002, 44.999], [2.002, 45.001]]);
    let points = trace(1.999, 2.004, 45.0, 0.0001);

    assert!(match_tunnels(&points, &[way], &TunnelParams::default()).is_empty());
}

#[test]
fn rejects_a_parallel_way_outside_the_distance_tolerance() {
    let way = east_way("w1", 45.0005, 2.0, 2.003);
    let points = trace(1.999, 2.004, 45.00002, 0.0001);

    assert!(match_tunnels(&points, &[way], &TunnelParams::default()).is_empty());
}

#[test]
fn filters_runs_shorter_than_the_minimum() {
    let way = east_way("w1", 45.0, 2.0, 2.003);
    let points = vec![
        [1.999, 45.00002],
        [2.0005, 45.00002],
        [2.0006, 45.00002],
        [2.004, 45.00002],
    ];

    let mut params = TunnelParams::default();
    let tunnels = match_tunnels(&points, &[way.clone()], &params);
    assert_eq!(tunnels.len(), 1, "deux points consécutifs suffisent");

    params.min_run_length = 3;
    assert!(
        match_tunnels(&points, &[way], &params).is_empty(),
        "un run de deux points doit être écarté avec minRunLength = 3"
    );
}

#[test]
fn splits_two_disjoint_runs_on_the_same_way() {
    let way = east_way("w1", 45.0, 2.0, 2.003);
    let points = vec![
        [2.0005, 45.00002],
        [2.0007, 45.00002],
        [2.0009, 45.00002],
        [2.0010, 45.0010],
        [2.0012, 45.00002],
        [2.0014, 45.00002],
    ];
    let mut params = TunnelParams::default();
    // Le saut nord fait sortir de la tolérance de distance ; on neutralise le
    // cap pour isoler la logique de runs.
    params.angle_tolerance_deg = 180.0;

    let tunnels = match_tunnels(&points, &[way], &params);

    assert_eq!(tunnels.len(), 2);
    assert_eq!(tunnels[0].way_id.as_deref(), Some("w1"));
    assert_eq!(tunnels[1].way_id.as_deref(), Some("w1"));
    assert!(tunnels[0].end_km < tunnels[1].start_km);
}

#[test]
fn chooses_the_closest_of_two_parallel_ways() {
    let close = east_way("w-close", 45.00002, 2.0, 2.003);
    let far = east_way("w-far", 45.0001, 2.0, 2.003);
    let points = trace(1.999, 2.004, 45.00002, 0.0001);

    let tunnels = match_tunnels(&points, &[far, close], &TunnelParams::default());

    assert!(!tunnels.is_empty());
    assert!(tunnels
        .iter()
        .all(|tunnel| tunnel.way_id.as_deref() == Some("w-close")));
}

#[test]
fn matches_a_trace_travelling_in_the_opposite_direction() {
    let way = east_way("w1", 45.0, 2.0, 2.003);
    let mut points = trace(2.0005, 2.0025, 45.00002, 0.0001);
    points.reverse();

    let tunnels = match_tunnels(&points, &[way], &TunnelParams::default());
    assert_eq!(tunnels.len(), 1);
}

#[test]
fn builds_the_tunnel_file() {
    let way = east_way("w1", 45.0, 2.0, 2.003);
    let points = trace(1.999, 2.004, 45.00002, 0.0001);
    let tunnels = match_tunnels(&points, &[way], &TunnelParams::default());

    let file = build_file(
        "trace-1",
        &TunnelParams::default(),
        vec![PbfFileRef {
            name: "fixture.osm.pbf".to_string(),
            modified_at: None,
        }],
        tunnels,
        Some("2026-10-22T10:00:00Z".to_string()),
        "2026-10-22T10:00:00Z",
    );

    assert_eq!(file.version, FILE_VERSION);
    assert_eq!(file.trace_id, "trace-1");
    assert_eq!(file.tunnels.len(), 1);
    assert_eq!(file.pbf_files.len(), 1);
    assert_eq!(file.params, TunnelParams::default());
}

#[test]
fn loads_points_from_a_real_gpx_fixture() {
    let path = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/gpx_multiride/tests/fixtures/poggio.gpx"
    ));
    let points = load_points(path).expect("lecture du GPX de test");

    assert!(points.len() > 2);
    assert!(points
        .iter()
        .all(|point| point[0].is_finite() && point[1].is_finite()));
}
