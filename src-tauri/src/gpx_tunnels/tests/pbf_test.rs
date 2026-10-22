//! Tests de lecture PBF et de fusion des ways.

use std::path::{Path, PathBuf};

use crate::gpx_tunnels::pbf::{merge_ways, read_tunnel_ways, OsmWay};
use std::collections::BTreeMap;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/gpx_tunnels/tests/fixtures/tunnel-sample.osm.pbf")
}

#[test]
fn reads_only_ways_that_are_both_tunnel_and_highway() {
    let ways = read_tunnel_ways(&fixture(), [1.99, 44.99, 2.01, 45.03])
        .expect("lecture du PBF de test");

    assert_eq!(ways.len(), 1, "seule la way 100 doit être retenue");
    let way = &ways[0];
    assert_eq!(way.id, "w100");
    assert_eq!(way.tags.get("highway").map(String::as_str), Some("primary"));
    assert_eq!(way.tags.get("tunnel").map(String::as_str), Some("yes"));
    assert_eq!(
        way.tags.get("tunnel:name").map(String::as_str),
        Some("Tunnel du Test")
    );
    assert_eq!(way.coords.len(), 3);
    assert_eq!(way.coords[0], [2.0, 45.0]);
    assert_eq!(way.coords[2], [2.002, 45.0]);
}

#[test]
fn bbox_outside_the_extract_returns_no_way() {
    let ways = read_tunnel_ways(&fixture(), [3.0, 46.0, 3.1, 46.1])
        .expect("lecture du PBF de test");
    assert!(ways.is_empty());
}

#[test]
fn merge_deduplicates_by_osm_id() {
    let ways = read_tunnel_ways(&fixture(), [1.99, 44.99, 2.01, 45.03])
        .expect("lecture du PBF de test");

    let mut duplicated = ways.clone();
    duplicated.extend(ways.clone());
    let merged = merge_ways(duplicated);
    assert_eq!(merged.len(), ways.len());

    let extra = OsmWay {
        id: "w999".to_string(),
        tags: BTreeMap::new(),
        coords: vec![[2.0, 45.0], [2.001, 45.0]],
    };
    let merged = merge_ways(vec![merged[0].clone(), extra]);
    assert_eq!(merged.len(), 2);
}

#[test]
#[ignore = "benchmark manuel : définir VISUGPS2_TUNNELS_BENCH_PBF"]
fn benchmark_real_pbf() {
    let path = std::env::var("VISUGPS2_TUNNELS_BENCH_PBF")
        .expect("définir VISUGPS2_TUNNELS_BENCH_PBF avec le chemin d'un PBF régional");
    let started = std::time::Instant::now();
    let ways = read_tunnel_ways(Path::new(&path), [-180.0, -90.0, 180.0, 90.0])
        .expect("lecture du PBF de benchmark");
    println!(
        "[bench] {} ways tunnel en {:?} ({})",
        ways.len(),
        started.elapsed(),
        path
    );
    assert!(!ways.is_empty());
}
