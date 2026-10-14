//! Tests des commandes du module Communes : écriture (détection) et diagnostic
//! (réinitialisation).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::gpx_communes::commands::{
    commit_detection, default_communes_params, detect_impl, reset_impl,
};
use crate::gpx_communes::file::{file_path, load_file};
use crate::import_gpx::{
    get_traces_path, load_registry, save_registry, Point3D, TraceMetadata, TraceStats,
};

// ─── Aides de test ────────────────────────────────────────────────────

/// Dossier temporaire isolé pour un test (nettoyé au préalable).
fn test_mode_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vg2_communes_cmd_{}_{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// Entrée de registre minimale : trace nettoyée, sans détection de communes.
fn make_trace(trace_id: &str) -> TraceMetadata {
    TraceMetadata {
        id: trace_id.to_string(),
        name: "Trace de test".to_string(),
        source: "Test".to_string(),
        source_url: None,
        activity_type: None,
        filename: "trace.gpx".to_string(),
        import_date: "2026-01-01T00:00:00Z".to_string(),
        stats: TraceStats {
            start_point: Point3D { lat: 45.0, lon: 2.0, alt: None },
            end_point: Point3D { lat: 45.0, lon: 2.02, alt: None },
            distance_m: 1574.0,
            positive_elevation_m: 0.0,
            negative_elevation_m: 0.0,
            alt_min_m: None,
            alt_max_m: None,
            points_count: 3,
            duration_s: None,
        },
        hash: "sha256:test".to_string(),
        favorite: false,
        is_displayed: false,
        audit_status: "clean".to_string(),
        audit_archived: false,
        multiride_status: None,
        communes_completed: false,
    }
}

/// Géométrie persistée d'une trace : LineString ouest → est à latitude constante
/// (`{mode}/traces/{id}/trace.geojson`, seul fichier lu par la détection).
fn write_trace_geojson(mode: &Path, trace_id: &str) {
    let dir = mode.join("traces").join(trace_id);
    fs::create_dir_all(&dir).expect("dossier de trace");
    let feature = json!({
        "type": "Feature",
        "properties": { "id": trace_id },
        "geometry": {
            "type": "LineString",
            "coordinates": [[2.0, 45.0], [2.01, 45.0], [2.02, 45.0]],
        },
    });
    fs::write(
        dir.join("trace.geojson"),
        serde_json::to_string(&feature).expect("sérialisation"),
    )
    .expect("écriture de la géométrie");
}

/// Réponse Overpass (tableau `elements`) : une commune couvrant toute la trace.
fn commune_payload(name: &str) -> String {
    let ring = json!([
        { "lat": 44.98, "lon": 1.99 },
        { "lat": 45.02, "lon": 1.99 },
        { "lat": 45.02, "lon": 2.03 },
        { "lat": 44.98, "lon": 2.03 },
        { "lat": 44.98, "lon": 1.99 },
    ]);
    serde_json::to_string(&json!([{
        "type": "relation",
        "id": 1,
        "tags": { "boundary": "administrative", "admin_level": "8", "name": name },
        "members": [{ "type": "way", "ref": 1, "role": "outer", "geometry": ring }],
    }]))
    .expect("sérialisation du payload")
}

/// Mode temporaire avec une trace : géométrie persistée + entrée de registre.
fn setup(name: &str, trace_id: &str) -> PathBuf {
    let mode = test_mode_dir(name);
    fs::create_dir_all(&mode).expect("dossier du mode");
    write_trace_geojson(&mode, trace_id);
    save_registry(&get_traces_path(&mode), &[make_trace(trace_id)]).expect("écriture du registre");
    mode
}

// ─── Détection ────────────────────────────────────────────────────────

/// La détection écrit la description **puis** pose le drapeau du registre.
#[test]
fn detect_writes_the_description_and_raises_the_flag() {
    let trace_id = "t-detect";
    let mode = setup("detect", trace_id);

    let passages = detect_impl(
        &mode,
        trace_id,
        &commune_payload("Test"),
        &default_communes_params(),
    )
    .expect("la détection doit aboutir");

    assert_eq!(passages.len(), 1);
    assert_eq!(passages[0].commune, "Test");
    assert_eq!(passages[0].km_entree, 0.0, "la trace démarre dans la commune");
    assert!(file_path(&mode, trace_id).exists(), "la description doit être écrite");
    assert_eq!(load_file(&mode, trace_id), Some(passages.clone()), "relisible telle quelle");
    assert!(
        load_registry(&get_traces_path(&mode))[0].communes_completed,
        "le drapeau doit être posé"
    );

    let _ = fs::remove_dir_all(&mode);
}

/// Une trace sans description ne rend rien à consulter — sans erreur.
#[test]
fn load_returns_nothing_without_a_description() {
    let trace_id = "t-none";
    let mode = setup("load_none", trace_id);

    assert_eq!(load_file(&mode, trace_id), None);

    let _ = fs::remove_dir_all(&mode);
}

// ─── Réinitialisation (diagnostic) ────────────────────────────────────

/// La réinitialisation supprime le fichier et repose le drapeau.
#[test]
fn reset_removes_the_file_and_clears_the_flag() {
    let trace_id = "t-reset";
    let mode = setup("reset", trace_id);
    detect_impl(
        &mode,
        trace_id,
        &commune_payload("Test"),
        &default_communes_params(),
    )
    .expect("détection préalable");
    assert!(file_path(&mode, trace_id).exists());

    reset_impl(&mode, trace_id).expect("la réinitialisation doit aboutir");

    assert!(
        !file_path(&mode, trace_id).exists(),
        "la description doit être supprimée"
    );
    assert!(
        !load_registry(&get_traces_path(&mode))[0].communes_completed,
        "le drapeau doit être reposé"
    );

    let _ = fs::remove_dir_all(&mode);
}

/// Une trace inconnue est refusée, et rien n'est touché.
#[test]
fn reset_rejects_an_unknown_trace() {
    let mode = test_mode_dir("reset_unknown");
    fs::create_dir_all(&mode).expect("dossier temporaire");

    let err = reset_impl(&mode, "absent").expect_err("une trace absente doit être refusée");

    assert!(err.contains("introuvable"), "message inattendu : {err}");
    let _ = fs::remove_dir_all(&mode);
}

// ─── Trace réécrite pendant l'analyse (volet de sûreté) ───────────────

/// Une trace dont le GPX a changé pendant l'analyse — un audit appliqué a
/// réécrit la géométrie — n'est **pas** marquée détectée : la description
/// calculée décrit l'ancienne trace. Rien n'est écrit, la détection sera rejouée.
#[test]
fn detection_is_abandoned_when_the_trace_changed_during_the_analysis() {
    let trace_id = "t-abandon";
    let mode = setup("abandon", trace_id);

    // Empreinte témoin différente de celle du registre : la trace a été réécrite.
    let abandonne = commit_detection(&mode, trace_id, &[], Some("sha256:empreinte-precedente"))
        .expect("l'appel doit aboutir");

    assert!(abandonne, "l'empreinte ne correspond plus : détection abandonnée");
    assert!(
        !file_path(&mode, trace_id).exists(),
        "aucune description ne doit être écrite"
    );
    assert!(
        !load_registry(&get_traces_path(&mode))[0].communes_completed,
        "le drapeau reste à false : la trace sera reprise"
    );

    // Empreinte conforme : la description est écrite et le drapeau posé.
    let empreinte = load_registry(&get_traces_path(&mode))[0].hash.clone();
    let abandonne = commit_detection(&mode, trace_id, &[], Some(&empreinte))
        .expect("l'appel doit aboutir");

    assert!(!abandonne);
    assert!(file_path(&mode, trace_id).exists(), "la description est écrite");
    assert!(load_registry(&get_traces_path(&mode))[0].communes_completed);

    let _ = fs::remove_dir_all(&mode);
}
