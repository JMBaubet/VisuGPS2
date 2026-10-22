//! Tests du module Tunnels.

mod commands_test;
mod download_test;
mod file_test;
mod geometry_test;
mod params_test;
mod pbf_test;
mod pipeline_test;
mod regions_test;

use super::types::{PbfFileRef, Tunnel, TunnelFile, TunnelParams, FILE_VERSION};

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
