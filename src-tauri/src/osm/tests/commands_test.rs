//! Tests des helpers des commandes OSM.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::gpx_tunnels::types::{RegionEntry, RegionsFile};
use crate::osm::commands::{catalog_from, check_download_limits, validate_filename};

fn temp_dir() -> PathBuf {
    std::env::temp_dir().join(format!("visugps2-osm-cmd-{}", Uuid::new_v4()))
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn region(id: &str, name: &str, filename: &str) -> RegionEntry {
    RegionEntry {
        id: id.to_string(),
        name: name.to_string(),
        country: "france".to_string(),
        parent: "france".to_string(),
        country_code: Some("FR".to_string()),
        country_name: Some("France".to_string()),
        bbox: [4.65, 42.98, 7.72, 45.13],
        pbf_filename: filename.to_string(),
        pbf_url: format!("https://example/{}", filename),
        size_mb: None,
    }
}

#[test]
fn accepts_plain_pbf_filenames() {
    assert!(validate_filename("paca-latest.osm.pbf").is_ok());
    assert!(validate_filename("monaco-latest.osm.pbf").is_ok());
}

#[test]
fn rejects_paths_non_pbf_and_the_catalog() {
    for filename in [
        "",
        ".",
        "..",
        ".pbf",
        "../x.pbf",
        "/tmp/x.pbf",
        "a/b.pbf",
        "a\\b.pbf",
        "regions.json",
        "x.txt",
        "x.pbf.part",
    ] {
        assert!(
            validate_filename(filename).is_err(),
            "nom refusé attendu : {:?}",
            filename
        );
    }
}

#[test]
fn download_limits_apply_the_cap_and_the_free_space() {
    let mb = 1_048_576_u64;

    assert!(check_download_limits("France", 100 * mb, 10_000 * mb, 500, false).is_ok());

    let cap = check_download_limits("France", 600 * mb, 10_000 * mb, 500, false)
        .expect_err("plafond dépassé");
    assert!(cap.contains("plafond"));

    assert!(check_download_limits("France", 600 * mb, 10_000 * mb, 500, true).is_ok());

    let space = check_download_limits("France", 100 * mb, 50 * mb, 500, false)
        .expect_err("espace insuffisant");
    assert!(space.contains("Espace insuffisant"));

    assert!(check_download_limits("France", 100 * mb, 10_000 * mb, 0, false).is_ok());
}

#[test]
fn catalog_reports_present_files_and_country() {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).expect("dossier temporaire");
    std::fs::write(dir.join("paca-latest.osm.pbf"), b"abc").expect("PBF présent");
    let file = RegionsFile {
        version: "v1".to_string(),
        regions: vec![
            region(
                "provence-alpes-cote-d-azur",
                "Provence-Alpes-Côte d'Azur",
                "paca-latest.osm.pbf",
            ),
            region("france", "France", "france-latest.osm.pbf"),
        ],
    };

    let catalog = catalog_from(&dir, &file);
    assert!(catalog.present);
    assert_eq!(catalog.version.as_deref(), Some("v1"));
    assert_eq!(catalog.regions.len(), 2);

    let paca = &catalog.regions[0];
    assert_eq!(paca.id, "provence-alpes-cote-d-azur");
    assert_eq!(paca.country_code.as_deref(), Some("FR"));
    assert_eq!(paca.country_name.as_deref(), Some("France"));
    assert!(paca.present);
    assert_eq!(paca.local_size_bytes, Some(3));
    assert!(paca.local_created_at.is_some());

    let france = &catalog.regions[1];
    assert!(!france.present);
    assert_eq!(france.local_size_bytes, None);
    assert_eq!(france.local_created_at, None);

    cleanup(&dir);
}
