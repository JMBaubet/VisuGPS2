//! Tests de l'index Geofabrik, de la sélection des régions et des ressources.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::gpx_tunnels::regions::{
    build_requirements, count_pbfs, find_regions, load_regions, pbf_present, save_regions,
    select_regions, transform_index,
};
use crate::gpx_tunnels::types::{
    RegionEntry, RegionsFile, READINESS_DISABLED, READINESS_MISSING_PBF,
    READINESS_NEEDS_REGIONS, READINESS_NOT_CONFIGURED, READINESS_NO_REGION,
    READINESS_READY,
};

fn temp_dir() -> PathBuf {
    std::env::temp_dir().join(format!("visugps2-regions-{}", Uuid::new_v4()))
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn region(id: &str, name: &str, bbox: [f64; 4], filename: &str) -> RegionEntry {
    RegionEntry {
        id: id.to_string(),
        name: name.to_string(),
        country: "europe".to_string(),
        parent: "europe".to_string(),
        bbox,
        pbf_filename: filename.to_string(),
        pbf_url: format!("https://example/{}", filename),
        size_mb: None,
    }
}

fn index_json() -> &'static str {
    r#"{
      "type": "FeatureCollection",
      "features": [
        {
          "type": "Feature",
          "geometry": {
            "type": "Polygon",
            "coordinates": [[[4.65, 42.98], [7.72, 42.98], [7.72, 45.13], [4.65, 45.13], [4.65, 42.98]]]
          },
          "properties": {
            "id": "europe/france/paca",
            "name": "Provence-Alpes-Côte d'Azur",
            "parent": "europe/france",
            "urls": { "pbf": "https://example/paca-latest.osm.pbf" }
          }
        },
        {
          "type": "Feature",
          "geometry": {
            "type": "MultiPolygon",
            "coordinates": [[[[7.40, 43.72], [7.44, 43.72], [7.44, 43.75], [7.40, 43.75], [7.40, 43.72]]]]
          },
          "properties": {
            "id": "europe/monaco",
            "name": "Monaco",
            "parent": "europe",
            "urls": { "pbf": "https://example/monaco-latest.osm.pbf" }
          }
        },
        {
          "type": "Feature",
          "geometry": null,
          "properties": {
            "id": "null-geometry",
            "name": "Sans géométrie",
            "parent": "europe",
            "urls": { "pbf": "https://example/null.osm.pbf" }
          }
        },
        {
          "type": "Feature",
          "geometry": {
            "type": "Polygon",
            "coordinates": [[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]]]
          },
          "properties": {
            "id": "sans-url",
            "name": "Sans URL",
            "parent": "europe"
          }
        }
      ]
    }"#
}

#[test]
fn transforms_a_geofabrik_index_into_light_regions() {
    let regions = transform_index(index_json(), "2026-10-22T10:00:00Z")
        .expect("transformation de l'index");

    assert_eq!(regions.version, "2026-10-22T10:00:00Z");
    assert_eq!(regions.regions.len(), 2);

    let paca = &regions.regions[0];
    assert_eq!(paca.id, "europe/france/paca");
    assert_eq!(paca.name, "Provence-Alpes-Côte d'Azur");
    assert_eq!(paca.parent, "europe/france");
    assert_eq!(paca.country, "europe/france");
    assert_eq!(paca.pbf_filename, "paca-latest.osm.pbf");
    assert_eq!(paca.bbox, [4.65, 42.98, 7.72, 45.13]);

    let monaco = &regions.regions[1];
    assert_eq!(monaco.id, "europe/monaco");
    assert_eq!(monaco.pbf_filename, "monaco-latest.osm.pbf");
    assert_eq!(monaco.bbox, [7.40, 43.72, 7.44, 43.75]);
}

#[test]
fn find_regions_is_exhaustive_including_sub_regions() {
    let regions = transform_index(index_json(), "v").expect("transformation");

    let in_monaco = find_regions(&regions, [7.41, 43.73, 7.42, 43.74]);
    let ids: Vec<&str> = in_monaco.iter().map(|region| region.id.as_str()).collect();
    assert_eq!(ids, vec!["europe/france/paca", "europe/monaco"]);

    let in_paca_only = find_regions(&regions, [5.0, 44.0, 5.1, 44.1]);
    assert_eq!(in_paca_only.len(), 1);
    assert_eq!(in_paca_only[0].id, "europe/france/paca");

    assert!(find_regions(&regions, [2.0, 41.0, 3.0, 42.0]).is_empty());
}

#[test]
fn select_regions_prefers_the_smallest_containing_region() {
    let regions = RegionsFile {
        version: "v1".to_string(),
        regions: vec![
            region(
                "alps",
                "Alps",
                [4.0, 43.0, 8.0, 46.0],
                "alps-latest.osm.pbf",
            ),
            region(
                "france",
                "France",
                [4.0, 42.0, 8.0, 46.0],
                "france-latest.osm.pbf",
            ),
            region(
                "paca",
                "PACA",
                [5.0, 43.0, 7.0, 45.0],
                "paca-latest.osm.pbf",
            ),
            region(
                "monaco",
                "Monaco",
                [7.40, 43.72, 7.44, 43.75],
                "monaco-latest.osm.pbf",
            ),
        ],
    };

    // Une trace en PACA ne doit pas tirer alps/france.
    let paca = select_regions(&regions, &[[5.5, 44.0]]);
    assert_eq!(
        paca.iter().map(|entry| entry.id.as_str()).collect::<Vec<_>>(),
        vec!["paca"]
    );

    // Une trace qui longe Monaco sélectionne PACA et Monaco, pas leurs aînés.
    let border = select_regions(&regions, &[[5.5, 44.0], [7.41, 43.73]]);
    assert_eq!(
        border
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        vec!["monaco", "paca"]
    );

    assert!(select_regions(&regions, &[[2.0, 41.0]]).is_empty());
}

#[test]
fn save_and_load_round_trip() {
    let dir = temp_dir();
    let regions = RegionsFile {
        version: "v1".to_string(),
        regions: vec![region(
            "europe/monaco",
            "Monaco",
            [7.40, 43.72, 7.44, 43.75],
            "monaco-latest.osm.pbf",
        )],
    };

    save_regions(&dir, &regions).expect("écriture de regions.json");
    let loaded = load_regions(&dir).expect("relecture de regions.json");
    assert_eq!(loaded, regions);

    cleanup(&dir);
}

#[test]
fn empty_regions_file_is_considered_missing() {
    let dir = temp_dir();
    save_regions(
        &dir,
        &RegionsFile {
            version: "v1".to_string(),
            regions: Vec::new(),
        },
    )
    .expect("écriture");
    assert!(load_regions(&dir).is_none());
    cleanup(&dir);
}

#[test]
fn pbf_presence_requires_a_non_empty_file() {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).expect("dossier temporaire");

    std::fs::write(dir.join("empty.osm.pbf"), b"").expect("fichier vide");
    assert!(!pbf_present(&dir, "empty.osm.pbf"));

    std::fs::write(dir.join("full.osm.pbf"), b"x").expect("fichier plein");
    assert!(pbf_present(&dir, "full.osm.pbf"));
    assert!(!pbf_present(&dir, "absent.osm.pbf"));

    cleanup(&dir);
}

#[test]
fn build_requirements_reports_missing_then_ready() {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).expect("dossier temporaire");
    let paca = region(
        "europe/france/paca",
        "PACA",
        [4.65, 42.98, 7.72, 45.13],
        "paca-latest.osm.pbf",
    );
    let monaco = region(
        "europe/monaco",
        "Monaco",
        [7.40, 43.72, 7.44, 43.75],
        "monaco-latest.osm.pbf",
    );
    let regions = RegionsFile {
        version: "v1".to_string(),
        regions: vec![paca, monaco],
    };
    let points = [[5.5, 44.0], [7.41, 43.73]];

    std::fs::write(dir.join("paca-latest.osm.pbf"), b"x").expect("PBF PACA");

    let requirements = build_requirements(true, Some(&dir), Some(&regions), &points);
    assert_eq!(requirements.readiness, READINESS_MISSING_PBF);
    assert_eq!(requirements.missing_count, 1);
    assert_eq!(requirements.regions.len(), 2);
    assert_eq!(
        requirements
            .regions
            .iter()
            .find(|entry| entry.id == "europe/france/paca")
            .map(|entry| entry.present),
        Some(true)
    );
    assert_eq!(
        requirements
            .regions
            .iter()
            .find(|entry| entry.id == "europe/monaco")
            .map(|entry| entry.present),
        Some(false)
    );
    assert!(requirements.message.is_some());

    std::fs::write(dir.join("monaco-latest.osm.pbf"), b"x").expect("PBF Monaco");
    let requirements = build_requirements(true, Some(&dir), Some(&regions), &points);
    assert_eq!(requirements.readiness, READINESS_READY);
    assert_eq!(requirements.missing_count, 0);
    assert!(requirements.regions.iter().all(|entry| entry.present));

    cleanup(&dir);
}

#[test]
fn build_requirements_covers_readiness_states() {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).expect("dossier temporaire");
    let regions = RegionsFile {
        version: "v1".to_string(),
        regions: vec![region(
            "europe/monaco",
            "Monaco",
            [7.40, 43.72, 7.44, 43.75],
            "monaco-latest.osm.pbf",
        )],
    };
    let points = [[7.41, 43.73]];

    assert_eq!(
        build_requirements(false, Some(&dir), Some(&regions), &points).readiness,
        READINESS_DISABLED
    );
    assert_eq!(
        build_requirements(true, None, Some(&regions), &points).readiness,
        READINESS_NOT_CONFIGURED
    );
    assert_eq!(
        build_requirements(true, Some(&dir), None, &points).readiness,
        READINESS_NEEDS_REGIONS
    );
    assert_eq!(
        build_requirements(true, Some(&dir), Some(&regions), &[[2.0, 41.0]]).readiness,
        READINESS_NO_REGION
    );

    cleanup(&dir);
}

#[test]
fn count_pbfs_ignores_other_files() {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).expect("dossier temporaire");
    std::fs::write(dir.join("a.osm.pbf"), b"x").expect("PBF A");
    std::fs::write(dir.join("b.osm.pbf"), b"x").expect("PBF B");
    std::fs::write(dir.join("a.osm.pbf.part"), b"x").expect("partiel");
    std::fs::write(dir.join("regions.json"), b"{}").expect("index");

    assert_eq!(count_pbfs(&dir), 2);
    cleanup(&dir);
}
