//! Tests du module Altimétrie.

mod file_test;
mod params_test;

use super::types::{
    AltimetrieCategoryDef, AltimetrieClimb, AltimetrieFile, AltimetrieLegend,
    AltimetrieMergedPart, AltimetrieParams, AltimetrieProfilePoint, AltimetrieSegment,
    AltimetrieTrace, FILE_VERSION,
};

/// Fichier d'exemple couvrant les deux formes de montées : une fusionnée non
/// séparée et un membre d'un groupe séparé.
pub fn sample_file(trace_id: &str) -> AltimetrieFile {
    AltimetrieFile {
        version: FILE_VERSION,
        trace_id: trace_id.to_string(),
        source: "trace.gpx".to_string(),
        updated_at: "2026-10-19T12:00:00Z".to_string(),
        params: AltimetrieParams::default(),
        trace: AltimetrieTrace {
            total_distance_m: 5_000.0,
            total_distance_sampled_m: 4_950.0,
            trailing_gap_m: 50.0,
            step_m: 50.0,
            total_points: 100,
        },
        profile: vec![
            AltimetrieProfilePoint {
                d: 0.0,
                ele: 1_000.0,
                lat: 45.0,
                lon: 6.0,
            },
            AltimetrieProfilePoint {
                d: 50.0,
                ele: 1_010.0,
                lat: 45.000_5,
                lon: 6.000_5,
            },
            AltimetrieProfilePoint {
                d: 100.0,
                ele: 1_020.0,
                lat: 45.001,
                lon: 6.001,
            },
        ],
        segments: vec![
            AltimetrieSegment {
                index: 0,
                start_km: 0.0,
                end_km: 0.05,
                start_progress: 0.0,
                end_progress: 0.01,
                category_id: "pl".to_string(),
                avg_slope_pct: 0.0,
                d_plus_m: 0.0,
            },
            AltimetrieSegment {
                index: 1,
                start_km: 0.05,
                end_km: 0.1,
                start_progress: 0.01,
                end_progress: 0.02,
                category_id: "up2".to_string(),
                avg_slope_pct: 5.0,
                d_plus_m: 10.0,
            },
        ],
        climbs: vec![
            AltimetrieClimb {
                id: 1,
                index: 0,
                start_km: 0.05,
                end_km: 0.1,
                start_progress: 0.01,
                end_progress: 0.02,
                d_plus_m: 10.0,
                avg_slope_pct: 5.0,
                alt_start_m: 1_010.0,
                alt_end_m: 1_020.0,
                merged_count: 1,
                fallback: false,
                is_merged: true,
                is_separated: false,
                group_id: None,
                merged_parts: Some(vec![
                    AltimetrieMergedPart {
                        part_index: 0,
                        start_km: 0.05,
                        end_km: 0.07,
                        start_progress: 0.01,
                        end_progress: 0.014,
                        d_plus_m: 6.0,
                        avg_slope_pct: 6.0,
                        alt_start_m: 1_010.0,
                        alt_end_m: 1_016.0,
                    },
                    AltimetrieMergedPart {
                        part_index: 1,
                        start_km: 0.08,
                        end_km: 0.1,
                        start_progress: 0.016,
                        end_progress: 0.02,
                        d_plus_m: 4.0,
                        avg_slope_pct: 4.0,
                        alt_start_m: 1_016.0,
                        alt_end_m: 1_020.0,
                    },
                ]),
            },
            AltimetrieClimb {
                id: 2,
                index: 1,
                start_km: 0.2,
                end_km: 0.25,
                start_progress: 0.04,
                end_progress: 0.05,
                d_plus_m: 8.0,
                avg_slope_pct: 4.0,
                alt_start_m: 1_030.0,
                alt_end_m: 1_038.0,
                merged_count: 0,
                fallback: false,
                is_merged: false,
                is_separated: true,
                group_id: Some(1),
                merged_parts: None,
            },
        ],
        legend: AltimetrieLegend {
            note: "Bornes hautes exclusives.".to_string(),
            fenetre_calcul: 100.0,
            longueur_min: 200.0,
            categories: vec![
                AltimetrieCategoryDef {
                    id: "dh3".to_string(),
                    label: "Descente raide".to_string(),
                    min_pct: None,
                    max_pct: Some(-7.0),
                },
                AltimetrieCategoryDef {
                    id: "up4".to_string(),
                    label: "Montée très raide".to_string(),
                    min_pct: Some(10.0),
                    max_pct: None,
                },
            ],
        },
    }
}
