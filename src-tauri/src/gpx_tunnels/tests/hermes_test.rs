//! Tests du moteur **hermes** du module Tunnels : conversion de la réponse du
//! serveur, requête de détection et commit local (statuts, couverture, garde
//! d'empreinte).

use crate::gpx_tunnels::file;
use crate::gpx_tunnels::hermes::{
    build_detection_request, commit_hermes_detection, tunnels_from_hermes,
};
use crate::gpx_tunnels::types::{
    group_count, TunnelParams, STATUS_OUTSIDE_COVERAGE, STATUS_SUCCESS,
    STATUS_WAITING_VALIDATE,
};
use crate::hermes::types::{HermesTunnel, HermesTunnelsDetectResponse};
use crate::import_gpx::{get_traces_path, load_registry, save_registry};

use super::{cleanup, sample_file, temp_mode_dir, trace_metadata, trace_metadata_with_hash};

// ─── Aides de test ────────────────────────────────────────────────────

fn served(
    id: &str,
    start_km: f64,
    end_km: f64,
    group_id: Option<&str>,
    way_ids: &[&str],
) -> HermesTunnel {
    HermesTunnel {
        id: id.to_string(),
        group_id: group_id.map(str::to_string),
        name: Some(format!("Tunnel {id}")),
        start_km,
        end_km,
        length_m: (end_km - start_km) * 1000.0,
        way_ids: way_ids.iter().map(|way| way.to_string()).collect(),
        altimetric_ok: None,
    }
}

fn response(dataset_id: &str, tunnels: Vec<HermesTunnel>) -> HermesTunnelsDetectResponse {
    HermesTunnelsDetectResponse {
        version: 1,
        trace_id: "trace-a".to_string(),
        dataset_id: dataset_id.to_string(),
        detected_at: "2026-11-25T10:00:00Z".to_string(),
        tunnels,
    }
}

fn registry_status(mode_dir: &std::path::Path, trace_id: &str) -> (Option<String>, Option<u32>) {
    let registry = load_registry(&get_traces_path(mode_dir));
    let entry = registry
        .iter()
        .find(|trace| trace.id == trace_id)
        .expect("trace du registre");
    (entry.tunnels_status.clone(), entry.tunnels_count)
}

// ─── Conversion ───────────────────────────────────────────────────────

#[test]
fn conversion_rounds_bounds_and_recomputes_length() {
    let converted = tunnels_from_hermes(&[served("tunnel-1", 1.234_56, 1.876_54, None, &["123"])]);

    assert_eq!(converted.len(), 1);
    let tunnel = &converted[0];
    assert_eq!(tunnel.start_km, 1.235);
    assert_eq!(tunnel.end_km, 1.877);
    assert_eq!(tunnel.length_m, 642.0);
    assert_eq!(tunnel.way_id.as_deref(), Some("123"));
    assert_eq!(tunnel.way_ids, vec!["123".to_string()]);
    assert_eq!(tunnel.detection_method, "osm");
    assert_eq!(tunnel.name.as_deref(), Some("Tunnel tunnel-1"));
}

#[test]
fn conversion_sorts_by_entry_and_ignores_server_groups() {
    let converted = tunnels_from_hermes(&[
        served("b", 2.0, 2.1, Some("g-1"), &[]),
        served("c", 3.0, 3.1, Some("solo"), &[]),
        served("a", 1.0, 1.1, Some("g-1"), &[]),
    ]);

    let ids: Vec<&str> = converted.iter().map(|tunnel| tunnel.id.as_str()).collect();
    assert_eq!(ids, vec!["a", "b", "c"]);
    assert!(converted.iter().all(|tunnel| tunnel.group_id.is_none()));
}

#[test]
fn conversion_does_not_merge_the_outbound_and_return_of_one_tunnel() {
    // Le serveur renvoie le même groupId pour les deux passages : ils doivent
    // rester deux tronçons distincts, sinon la carte trace une bande continue
    // sur tout l'aller-retour.
    let converted = tunnels_from_hermes(&[
        served("out", 24.947, 25.027, Some("g-153784146"), &["153784146"]),
        served("back", 105.66, 105.751, Some("g-153784146"), &["153784146"]),
    ]);

    assert_eq!(converted.len(), 2);
    assert!(converted.iter().all(|tunnel| tunnel.group_id.is_none()));
    assert_eq!(converted[0].start_km, 24.947);
    assert_eq!(converted[1].start_km, 105.66);
    assert_eq!(group_count(&converted), 2);
}

#[test]
fn conversion_replaces_empty_and_duplicate_ids() {
    let converted = tunnels_from_hermes(&[
        served("dup", 1.0, 1.1, None, &[]),
        served("dup", 2.0, 2.1, None, &[]),
        served("", 3.0, 3.1, None, &[]),
    ]);

    let ids: Vec<&str> = converted.iter().map(|tunnel| tunnel.id.as_str()).collect();
    assert_eq!(converted.len(), 3);
    assert_eq!(ids[0], "dup");
    assert_ne!(ids[1], "dup");
    assert!(ids[1].starts_with("t-"));
    assert!(ids[2].starts_with("t-"));
    assert_ne!(ids[1], ids[2]);
}

#[test]
fn conversion_ignores_invalid_bounds() {
    let converted = tunnels_from_hermes(&[
        served("zero", 5.0, 5.0, None, &[]),
        served("inverted", 6.0, 5.0, None, &[]),
        served("nan", f64::NAN, 7.0, None, &[]),
        served("ok", 8.0, 8.1, None, &[]),
    ]);

    assert_eq!(converted.len(), 1);
    assert_eq!(converted[0].id, "ok");
}

#[test]
fn build_request_transmits_min_length_and_disabled_altimetric_check() {
    let params = TunnelParams {
        min_length_m: 50.0,
        ..TunnelParams::default()
    };

    let request = build_detection_request(
        "trace-a",
        "ds-test",
        vec![[7.42, 43.73], [7.43, 43.74]],
        &params,
    );

    assert_eq!(request.trace_id, "trace-a");
    assert_eq!(request.dataset_id, "ds-test");
    assert_eq!(request.points.len(), 2);
    assert_eq!(request.params.min_length_m, 50.0);
    assert!(!request.params.altimetric_check);
    assert_eq!(request.params.altimetric_tolerance_m, 30.0);
}

// ─── Contrat serveur ──────────────────────────────────────────────────

#[test]
fn response_deserializes_the_server_contract() {
    let json = r#"{
      "version": 1,
      "traceId": "t-tunnel-01",
      "datasetId": "test-monaco-ds",
      "detectedAt": "2026-11-09T20:06:23Z",
      "params": {
        "minLengthM": 50.0,
        "altimetricCheck": false,
        "altimetricToleranceM": 30.0
      },
      "tunnels": [
        {
          "id": "abc",
          "groupId": "g1",
          "name": "Tunnel Rainier III",
          "startKm": 1.234,
          "endKm": 1.876,
          "lengthM": 642.0,
          "wayIds": ["164338236"],
          "altimetricOk": null
        }
      ]
    }"#;

    let response: HermesTunnelsDetectResponse =
        serde_json::from_str(json).expect("contrat serveur lisible");

    assert_eq!(response.trace_id, "t-tunnel-01");
    assert_eq!(response.dataset_id, "test-monaco-ds");
    assert_eq!(response.detected_at, "2026-11-09T20:06:23Z");
    assert_eq!(response.tunnels.len(), 1);
    let tunnel = &response.tunnels[0];
    assert_eq!(tunnel.group_id.as_deref(), Some("g1"));
    assert_eq!(tunnel.name.as_deref(), Some("Tunnel Rainier III"));
    assert_eq!(tunnel.start_km, 1.234);
    assert_eq!(tunnel.end_km, 1.876);
    assert_eq!(tunnel.way_ids, vec!["164338236".to_string()]);
    assert_eq!(tunnel.altimetric_ok, None);
}

// ─── Commit ───────────────────────────────────────────────────────────

#[test]
fn commit_writes_the_file_and_waits_for_validation() {
    let mode_dir = temp_mode_dir();
    save_registry(
        &get_traces_path(&mode_dir),
        &[trace_metadata("trace-a")],
    )
    .expect("écriture du registre");

    let result = commit_hermes_detection(
        &mode_dir,
        "trace-a",
        &response(
            "ds-test",
            vec![
                served("a", 1.0, 1.1, Some("g-1"), &["123"]),
                served("b", 2.0, 2.1, Some("g-1"), &["456"]),
            ],
        ),
        false,
        Some("sha256:test"),
        &TunnelParams::default(),
    )
    .expect("commit valide");

    assert_eq!(result.status, STATUS_WAITING_VALIDATE);
    assert!(!result.outside_coverage);
    assert_eq!(result.dataset_id, "ds-test");
    assert_eq!(result.file.tunnels.len(), 2);
    assert!(result.file.pbf_files.is_empty());
    assert!(result
        .file
        .tunnels
        .iter()
        .all(|tunnel| tunnel.group_id.is_none()));
    assert_eq!(result.file.detected_at.as_deref(), Some("2026-11-25T10:00:00Z"));

    let stored = file::load_file(&file::file_path(&mode_dir, "trace-a"), "trace-a")
        .expect("fichier écrit");
    assert_eq!(stored.tunnels.len(), 2);
    let (status, count) = registry_status(&mode_dir, "trace-a");
    assert_eq!(status.as_deref(), Some(STATUS_WAITING_VALIDATE));
    assert_eq!(count, Some(2));

    cleanup(&mode_dir);
}

#[test]
fn commit_records_an_empty_result_as_success() {
    let mode_dir = temp_mode_dir();
    save_registry(
        &get_traces_path(&mode_dir),
        &[trace_metadata("trace-a")],
    )
    .expect("écriture du registre");

    let result = commit_hermes_detection(
        &mode_dir,
        "trace-a",
        &response("ds-test", Vec::new()),
        false,
        Some("sha256:test"),
        &TunnelParams::default(),
    )
    .expect("commit valide");

    assert_eq!(result.status, STATUS_SUCCESS);
    assert!(result.file.tunnels.is_empty());
    let stored = file::load_file(&file::file_path(&mode_dir, "trace-a"), "trace-a")
        .expect("fichier écrit");
    assert!(stored.tunnels.is_empty());
    let (status, count) = registry_status(&mode_dir, "trace-a");
    assert_eq!(status.as_deref(), Some(STATUS_SUCCESS));
    assert_eq!(count, Some(0));

    cleanup(&mode_dir);
}

#[test]
fn commit_records_an_empty_result_outside_coverage() {
    let mode_dir = temp_mode_dir();
    save_registry(
        &get_traces_path(&mode_dir),
        &[trace_metadata("trace-a")],
    )
    .expect("écriture du registre");

    let result = commit_hermes_detection(
        &mode_dir,
        "trace-a",
        &response("ds-test", Vec::new()),
        true,
        Some("sha256:test"),
        &TunnelParams::default(),
    )
    .expect("commit valide");

    assert_eq!(result.status, STATUS_OUTSIDE_COVERAGE);
    assert!(result.outside_coverage);
    let stored = file::load_file(&file::file_path(&mode_dir, "trace-a"), "trace-a")
        .expect("fichier vide écrit");
    assert!(stored.tunnels.is_empty());
    let (status, count) = registry_status(&mode_dir, "trace-a");
    assert_eq!(status.as_deref(), Some(STATUS_OUTSIDE_COVERAGE));
    assert_eq!(count, Some(0));

    cleanup(&mode_dir);
}

#[test]
fn commit_refuses_to_overwrite_an_existing_file_outside_coverage() {
    let mode_dir = temp_mode_dir();
    let existing = sample_file("trace-a");
    file::save_file(&file::file_path(&mode_dir, "trace-a"), &existing)
        .expect("écriture du fichier existant");
    save_registry(
        &get_traces_path(&mode_dir),
        &[trace_metadata("trace-a")],
    )
    .expect("écriture du registre");

    let error = commit_hermes_detection(
        &mode_dir,
        "trace-a",
        &response("ds-test", Vec::new()),
        true,
        Some("sha256:test"),
        &TunnelParams::default(),
    )
    .expect_err("le fichier existant doit être conservé");

    assert!(error.contains("hors de la couverture"));
    let stored = file::load_file(&file::file_path(&mode_dir, "trace-a"), "trace-a")
        .expect("fichier conservé");
    assert_eq!(stored.tunnels.len(), existing.tunnels.len());
    let (status, _) = registry_status(&mode_dir, "trace-a");
    assert_eq!(status, None);

    cleanup(&mode_dir);
}

#[test]
fn commit_abandons_the_detection_when_the_trace_was_rewritten() {
    let mode_dir = temp_mode_dir();
    save_registry(
        &get_traces_path(&mode_dir),
        &[trace_metadata_with_hash("trace-a", "sha256:new")],
    )
    .expect("écriture du registre");

    let error = commit_hermes_detection(
        &mode_dir,
        "trace-a",
        &response("ds-test", vec![served("a", 1.0, 1.1, None, &[])]),
        false,
        Some("sha256:old"),
        &TunnelParams::default(),
    )
    .expect_err("l'empreinte a changé");

    assert!(error.contains("modifiée pendant la détection"));
    assert!(
        file::load_file(&file::file_path(&mode_dir, "trace-a"), "trace-a").is_none(),
        "aucun fichier ne doit être écrit"
    );
    let (status, _) = registry_status(&mode_dir, "trace-a");
    assert_eq!(status, None);

    cleanup(&mode_dir);
}
