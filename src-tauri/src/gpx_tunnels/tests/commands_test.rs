//! Tests de validation et de normalisation des tunnels sauvegardés.

use crate::gpx_tunnels::commands::{backfill_state, group_count, normalize_tunnels};
use crate::gpx_tunnels::file;
use crate::gpx_tunnels::types::Tunnel;
use crate::import_gpx::{get_traces_path, load_registry, save_registry};

use super::{cleanup, sample_file, temp_mode_dir, trace_metadata};

fn tunnel(id: &str, start_km: f64, end_km: f64, group: Option<&str>) -> Tunnel {
    Tunnel {
        id: id.to_string(),
        way_id: None,
        way_ids: Vec::new(),
        name: None,
        highway: None,
        tunnel_type: None,
        start_km,
        end_km,
        length_m: 0.0,
        detection_method: "manual".to_string(),
        max_distance_m: None,
        max_angle_diff_deg: None,
        group_id: group.map(str::to_string),
    }
}

#[test]
fn normalizes_sorts_and_recomputes_lengths() {
    let input = vec![
        tunnel("t-3", 2.4, 2.5, Some("g-1")),
        tunnel("t-1", 2.0, 2.1, None),
        tunnel("t-2", 2.2, 2.3, Some("g-1")),
    ];

    let result = normalize_tunnels(input, 10.0).expect("tunnels valides");

    let ids: Vec<&str> = result.iter().map(|tunnel| tunnel.id.as_str()).collect();
    assert_eq!(ids, vec!["t-1", "t-2", "t-3"]);
    assert!(result.iter().all(|tunnel| tunnel.length_m == 100.0));
    assert_eq!(group_count(&result), 2);
}

#[test]
fn rejects_overlapping_tunnels() {
    let input = vec![
        tunnel("t-1", 2.0, 2.1, None),
        tunnel("t-2", 2.05, 2.2, None),
    ];

    assert!(normalize_tunnels(input, 10.0).is_err());
}

#[test]
fn accepts_touching_tunnels() {
    let input = vec![
        tunnel("t-1", 2.0, 2.1, None),
        tunnel("t-2", 2.1, 2.2, None),
    ];

    let result = normalize_tunnels(input, 10.0).expect("bornes jointives admises");
    assert_eq!(result.len(), 2);
    assert_eq!(group_count(&result), 2);
}

#[test]
fn rejects_a_too_short_tunnel() {
    let input = vec![tunnel("t-1", 2.0, 2.005, None)];
    assert!(normalize_tunnels(input, 10.0).is_err());
}

#[test]
fn rejects_a_group_with_a_single_member() {
    let input = vec![tunnel("t-1", 2.0, 2.1, Some("g-1"))];
    assert!(normalize_tunnels(input, 10.0).is_err());
}

#[test]
fn rejects_duplicate_ids() {
    let input = vec![
        tunnel("t-1", 2.0, 2.1, None),
        tunnel("t-1", 2.2, 2.3, None),
    ];
    assert!(normalize_tunnels(input, 10.0).is_err());
}

#[test]
fn rejects_a_tunnel_beyond_the_trace() {
    let input = vec![tunnel("t-1", 9.0, 11.0, None)];
    assert!(normalize_tunnels(input, 10.0).is_err());
}

#[test]
fn rejects_negative_bounds() {
    let input = vec![tunnel("t-1", -0.1, 0.1, None)];
    assert!(normalize_tunnels(input, 10.0).is_err());
}

#[test]
fn normalizes_an_unknown_detection_method_to_manual() {
    let mut value = tunnel("t-1", 2.0, 2.1, None);
    value.detection_method = "imported".to_string();

    let result = normalize_tunnels(vec![value], 10.0).expect("tunnel valide");
    assert_eq!(result[0].detection_method, "manual");
}

#[test]
fn group_count_counts_groups_once_and_singles_separately() {
    assert_eq!(group_count(&[]), 0);
    assert_eq!(group_count(&[tunnel("a", 0.0, 0.1, None)]), 1);
    assert_eq!(
        group_count(&[
            tunnel("a", 0.0, 0.1, Some("g")),
            tunnel("b", 0.2, 0.3, Some("g")),
            tunnel("c", 0.4, 0.5, None),
        ]),
        2
    );
}

#[test]
fn backfill_sets_state_for_existing_tunnel_files_only() {
    let mode_dir = temp_mode_dir();
    let sample = sample_file("trace-a");
    file::save_file(&file::file_path(&mode_dir, "trace-a"), &sample)
        .expect("écriture de tunnels.json");

    save_registry(
        &get_traces_path(&mode_dir),
        &[trace_metadata("trace-a"), trace_metadata("trace-b")],
    )
    .expect("écriture du registre");

    backfill_state(&mode_dir);

    let registry = load_registry(&get_traces_path(&mode_dir));
    let a = registry
        .iter()
        .find(|trace| trace.id == "trace-a")
        .expect("trace a");
    assert_eq!(a.tunnels_status.as_deref(), Some("waiting_validate"));
    assert_eq!(a.tunnels_count, Some(group_count(&sample.tunnels)));

    let b = registry
        .iter()
        .find(|trace| trace.id == "trace-b")
        .expect("trace b");
    assert_eq!(b.tunnels_status, None);
    assert_eq!(b.tunnels_count, None);

    cleanup(&mode_dir);
}
