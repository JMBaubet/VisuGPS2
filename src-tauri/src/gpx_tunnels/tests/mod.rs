//! Tests du module Tunnels.

mod commands_test;
mod download_test;
mod file_test;
mod geometry_test;
mod hermes_test;
mod params_test;
mod pbf_test;
mod pipeline_test;
mod regions_test;

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::import_gpx::{Point3D, TraceMetadata, TraceStats};

use super::types::{PbfFileRef, Tunnel, TunnelFile, TunnelParams, FILE_VERSION};

/// Dossier de mode temporaire, propre à un test. Il est créé : le registre et
/// les dossiers de trace sont écrits par les tests eux-mêmes.
pub fn temp_mode_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("visugps2-tunnel-tests-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("création du dossier de test");
    dir
}

pub fn cleanup(mode_dir: &Path) {
    let _ = std::fs::remove_dir_all(mode_dir);
}

/// Entrée de registre minimale pour les tests de commit et de rattrapage.
pub fn trace_metadata(trace_id: &str) -> TraceMetadata {
    trace_metadata_with_hash(trace_id, "sha256:test")
}

/// Variante avec empreinte explicite, pour tester la garde d'écriture.
pub fn trace_metadata_with_hash(trace_id: &str, hash: &str) -> TraceMetadata {
    TraceMetadata {
        id: trace_id.to_string(),
        name: "Trace de test".to_string(),
        source: "test".to_string(),
        source_url: None,
        activity_type: None,
        filename: "trace.gpx".to_string(),
        import_date: "2026-10-22T10:00:00Z".to_string(),
        stats: TraceStats {
            start_point: Point3D {
                lat: 45.0,
                lon: 2.0,
                alt: Some(0.0),
            },
            end_point: Point3D {
                lat: 45.0,
                lon: 2.0,
                alt: Some(0.0),
            },
            distance_m: 1000.0,
            positive_elevation_m: 0.0,
            negative_elevation_m: 0.0,
            alt_min_m: Some(0.0),
            alt_max_m: Some(0.0),
            points_count: 10,
            duration_s: None,
        },
        hash: hash.to_string(),
        favorite: false,
        is_displayed: false,
        audit_status: "clean".to_string(),
        audit_archived: false,
        multiride_status: None,
        multiride_complete: None,
        communes_completed: false,
        communes_status: None,
        altimetrie_validated: false,
        altimetrie_climb_count: None,
        tunnels_status: None,
        tunnels_count: None,
    }
}

/// Fichier d'exemple couvrant un tunnel isolé et un groupe fusionné de deux
/// tronçons.
pub fn sample_file(trace_id: &str) -> TunnelFile {
    TunnelFile {
        version: FILE_VERSION,
        trace_id: trace_id.to_string(),
        detected_at: Some("2026-10-22T10:00:00Z".to_string()),
        updated_at: "2026-10-22T10:05:00Z".to_string(),
        pbf_files: vec![PbfFileRef {
            name: "provence-alpes-cote-d-azur-latest.osm.pbf".to_string(),
            modified_at: Some("2026-10-20T08:00:00Z".to_string()),
        }],
        params: TunnelParams::default(),
        tunnels: vec![
            Tunnel {
                id: "t-001".to_string(),
                way_id: Some("w123456".to_string()),
                way_ids: vec!["w123456".to_string()],
                name: Some("Tunnel de la Giraude".to_string()),
                highway: Some("motorway".to_string()),
                tunnel_type: Some("yes".to_string()),
                start_km: 12.34,
                end_km: 13.01,
                length_m: 670.0,
                detection_method: "osm".to_string(),
                max_distance_m: Some(8.4),
                max_angle_diff_deg: Some(6.0),
                group_id: None,
            },
            Tunnel {
                id: "t-002".to_string(),
                way_id: Some("w234567".to_string()),
                way_ids: vec!["w234567".to_string()],
                name: Some("Tunnel du Peyronnet".to_string()),
                highway: Some("motorway".to_string()),
                tunnel_type: Some("yes".to_string()),
                start_km: 20.0,
                end_km: 20.12,
                length_m: 120.0,
                detection_method: "osm".to_string(),
                max_distance_m: Some(5.1),
                max_angle_diff_deg: Some(4.0),
                group_id: Some("g-1".to_string()),
            },
            Tunnel {
                id: "t-003".to_string(),
                way_id: None,
                way_ids: Vec::new(),
                name: None,
                highway: None,
                tunnel_type: None,
                start_km: 20.2,
                end_km: 20.25,
                length_m: 50.0,
                detection_method: "manual".to_string(),
                max_distance_m: None,
                max_angle_diff_deg: None,
                group_id: Some("g-1".to_string()),
            },
        ],
    }
}
