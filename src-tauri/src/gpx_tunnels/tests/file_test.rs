//! Tests du fichier `tunnels.json` : aller-retour, lecture tolérante et
//! suppression.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::gpx_tunnels::file;
use crate::gpx_tunnels::types::FILE_VERSION;

use super::sample_file;

fn temp_mode_dir() -> PathBuf {
    std::env::temp_dir().join(format!("visugps2-tunnels-{}", Uuid::new_v4()))
}

fn cleanup(mode_dir: &Path) {
    let _ = std::fs::remove_dir_all(mode_dir);
}

#[test]
fn save_and_load_round_trip() {
    let mode_dir = temp_mode_dir();
    let trace_id = "trace-1";
    let path = file::file_path(&mode_dir, trace_id);
    let expected = sample_file(trace_id);

    file::save_file(&path, &expected).expect("écriture du fichier");
    assert!(path.exists(), "le fichier doit exister après écriture");
    assert!(
        !path.with_extension("json.tmp").exists(),
        "aucun fichier temporaire ne doit subsister"
    );

    let loaded = file::load_file(&path, trace_id).expect("relecture du fichier");
    assert_eq!(loaded, expected, "l'aller-retour doit préserver chaque champ");

    cleanup(&mode_dir);
}

#[test]
fn load_ignores_unknown_version() {
    let mode_dir = temp_mode_dir();
    let trace_id = "trace-version";
    let path = file::file_path(&mode_dir, trace_id);
    let mut file_on_disk = sample_file(trace_id);
    file_on_disk.version = FILE_VERSION + 1;

    file::save_file(&path, &file_on_disk).expect("écriture du fichier");
    assert!(
        file::load_file(&path, trace_id).is_none(),
        "une version inconnue doit être ignorée"
    );

    cleanup(&mode_dir);
}

#[test]
fn load_ignores_other_trace() {
    let mode_dir = temp_mode_dir();
    let path = file::file_path(&mode_dir, "trace-a");
    file::save_file(&path, &sample_file("trace-a")).expect("écriture du fichier");

    assert!(
        file::load_file(&path, "trace-b").is_none(),
        "un fichier rattaché à une autre trace doit être ignoré"
    );

    cleanup(&mode_dir);
}

#[test]
fn load_returns_none_for_missing_file() {
    let mode_dir = temp_mode_dir();
    let path = file::file_path(&mode_dir, "absente");
    assert!(file::load_file(&path, "absente").is_none());
    cleanup(&mode_dir);
}

#[test]
fn remove_file_is_tolerant() {
    let mode_dir = temp_mode_dir();
    let trace_id = "trace-remove";
    let path = file::file_path(&mode_dir, trace_id);
    file::save_file(&path, &sample_file(trace_id)).expect("écriture du fichier");
    assert!(path.exists());

    file::remove_file(&mode_dir, trace_id);
    assert!(!path.exists(), "le fichier doit être supprimé");

    // Deuxième suppression : fichier absent, aucune panique.
    file::remove_file(&mode_dir, trace_id);

    cleanup(&mode_dir);
}
