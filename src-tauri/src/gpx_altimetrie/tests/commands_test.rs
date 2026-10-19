//! Tests des implémentations de commandes Altimétrie : détection, aperçu sans
//! écriture, sauvegarde et génération automatique best-effort.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::gpx_altimetrie::commands::{
    detect_impl, generate_status, read_raw_points, rebuild_impl,
};
use crate::gpx_altimetrie::file;
use crate::gpx_altimetrie::types::{AltimetrieClimbEdit, AltimetrieParams};

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/gpx_multiride/tests/fixtures/poggio.gpx")
}

fn temp_mode_dir() -> PathBuf {
    std::env::temp_dir().join(format!("visugps2-altimetrie-commandes-{}", Uuid::new_v4()))
}

fn copy_fixture(mode_dir: &Path, trace_id: &str) -> PathBuf {
    let trace_dir = mode_dir.join("traces").join(trace_id);
    std::fs::create_dir_all(&trace_dir).expect("dossier de trace");
    let destination = trace_dir.join("poggio.gpx");
    std::fs::copy(fixture_path(), &destination).expect("copie de la fixture");
    destination
}

fn cleanup(mode_dir: &Path) {
    let _ = std::fs::remove_dir_all(mode_dir);
}

fn manual_climb(id: u32, start_km: f64, end_km: f64) -> AltimetrieClimbEdit {
    AltimetrieClimbEdit {
        id,
        start_km,
        end_km,
        is_merged: false,
        is_separated: false,
        group_id: None,
        merged_parts: None,
        fallback: false,
    }
}

#[test]
fn read_raw_points_extracts_elevations() {
    let points = read_raw_points(&fixture_path()).expect("lecture de la fixture");
    assert!(points.len() > 100);
    assert!(points.iter().all(|point| point.ele.is_finite()));

    assert!(
        read_raw_points(Path::new("/ce/fichier/nexiste/pas.gpx")).is_err(),
        "un GPX absent doit être refusé"
    );
}

#[test]
fn detect_writes_and_loads_the_file() {
    let mode_dir = temp_mode_dir();
    let trace_id = "trace-detect";
    let gpx_path = copy_fixture(&mode_dir, trace_id);
    let params = AltimetrieParams::default();

    let detected = detect_impl(
        &mode_dir,
        trace_id,
        &gpx_path,
        "poggio.gpx",
        &params,
        "2026-10-19T12:00:00Z",
    )
    .expect("détection");

    let path = file::file_path(&mode_dir, trace_id);
    let loaded = file::load_file(&path, trace_id).expect("relecture");
    assert_eq!(loaded, detected);
    assert_eq!(loaded.updated_at, "2026-10-19T12:00:00Z");
    assert!(loaded.profile.len() >= 2);
    assert!(!loaded.segments.is_empty());

    cleanup(&mode_dir);
}

#[test]
fn preview_does_not_write_and_save_writes() {
    let mode_dir = temp_mode_dir();
    let trace_id = "trace-save";
    let gpx_path = copy_fixture(&mode_dir, trace_id);
    let params = AltimetrieParams::default();
    let path = file::file_path(&mode_dir, trace_id);

    let detected = detect_impl(
        &mode_dir,
        trace_id,
        &gpx_path,
        "poggio.gpx",
        &params,
        "2026-10-19T12:00:00Z",
    )
    .expect("détection");
    let before = std::fs::read_to_string(&path).expect("contenu initial");

    let total_km = detected.trace.total_distance_m / 1_000.0;
    let climb = manual_climb(1, total_km * 0.1, total_km * 0.2);

    let preview = rebuild_impl(
        &mode_dir,
        trace_id,
        &gpx_path,
        "poggio.gpx",
        &detected.params,
        &detected.updated_at,
        &[climb.clone()],
        false,
    )
    .expect("aperçu");
    assert_eq!(preview.updated_at, detected.updated_at);
    assert_eq!(preview.climbs.len(), 1);
    assert!(
        std::fs::read_to_string(&path).expect("contenu après aperçu") == before,
        "l'aperçu ne doit rien écrire"
    );

    let saved = rebuild_impl(
        &mode_dir,
        trace_id,
        &gpx_path,
        "poggio.gpx",
        &detected.params,
        "2026-10-19T13:00:00Z",
        &[climb],
        true,
    )
    .expect("sauvegarde");
    assert_eq!(saved.updated_at, "2026-10-19T13:00:00Z");

    let reloaded = file::load_file(&path, trace_id).expect("relecture après sauvegarde");
    assert_eq!(reloaded.updated_at, "2026-10-19T13:00:00Z");
    assert_eq!(reloaded.climbs.len(), 1);
    assert!(reloaded.climbs[0].end_km > reloaded.climbs[0].start_km);

    cleanup(&mode_dir);
}

#[test]
fn generate_status_is_best_effort_and_invalidates_a_stale_file() {
    let mode_dir = temp_mode_dir();
    let trace_id = "trace-generate";
    let gpx_path = copy_fixture(&mode_dir, trace_id);
    let params = AltimetrieParams::default();
    let path = file::file_path(&mode_dir, trace_id);

    assert!(generate_status(
        &mode_dir,
        trace_id,
        &gpx_path,
        "poggio.gpx",
        params.clone()
    ));
    assert!(file::load_file(&path, trace_id).is_some());

    // GPX disparu : la génération échoue sans panique et retire le fichier
    // périmé, ce qui force une nouvelle tentative à l'ouverture de la vue.
    std::fs::remove_file(&gpx_path).expect("suppression du GPX");
    assert!(!generate_status(
        &mode_dir,
        trace_id,
        &gpx_path,
        "poggio.gpx",
        params
    ));
    assert!(file::load_file(&path, trace_id).is_none());

    cleanup(&mode_dir);
}
