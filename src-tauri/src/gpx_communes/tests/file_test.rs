//! Tests de l'accès disque du module Communes (`communes.json`).

use std::fs;
use std::path::{Path, PathBuf};

use crate::gpx_communes::file::{file_path, remove_file};

/// Dossier temporaire isolé pour un test (nettoyé au préalable).
fn test_mode_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vg2_communes_{}_{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// Le fichier vit dans le dossier de la trace, sous un nom uniforme.
#[test]
fn file_path_is_in_the_trace_directory() {
    let mode = PathBuf::from("/mode");
    let path = file_path(&mode, "abc-123");

    assert!(path.starts_with(&mode), "chemin hors du mode : {path:?}");
    assert_eq!(
        path.parent(),
        Some(Path::new("/mode/traces/abc-123")),
        "le fichier doit vivre dans le dossier de la trace"
    );
    assert_eq!(path.file_name().and_then(|n| n.to_str()), Some("communes.json"));
}

/// Suppression tolérante : absente, présente, puis déjà supprimée.
#[test]
fn remove_file_is_tolerant_and_idempotent() {
    let mode = test_mode_dir("remove");
    let trace_id = "t-1";
    let path = file_path(&mode, trace_id);

    // Absent : aucune erreur, aucun fichier créé.
    remove_file(&mode, trace_id);
    assert!(!path.exists());

    // Présent : supprimé.
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "[]").unwrap();
    remove_file(&mode, trace_id);
    assert!(!path.exists(), "le fichier doit avoir été supprimé");

    // Deuxième appel : toujours sans erreur.
    remove_file(&mode, trace_id);
    assert!(!path.exists());

    let _ = fs::remove_dir_all(&mode);
}
