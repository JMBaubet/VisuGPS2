//! Tests de l'accès disque du module Communes (`communes.json`).

use std::fs;
use std::path::{Path, PathBuf};

use crate::gpx_communes::file::{file_path, remove_file, save_file};
use crate::gpx_communes::types::CommunePassage;

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

/// Le fichier écrit respecte le contrat de la spécification : tableau racine
/// d'objets à six clés, indentation de 2 espaces, aucune géométrie.
#[test]
fn save_file_writes_the_documented_format() {
    let mode = test_mode_dir("save");
    let passages = vec![
        CommunePassage {
            commune: "Nice".to_string(),
            passage: 1,
            nb_passages_total: 1,
            km_entree: 0.0,
            km_sortie: 5.131,
            longueur_km: 5.131,
        },
        CommunePassage {
            commune: "La Trinité".to_string(),
            passage: 1,
            nb_passages_total: 1,
            km_entree: 5.131,
            km_sortie: 8.001,
            longueur_km: 2.87,
        },
    ];

    save_file(&mode, "t-1", &passages).expect("écriture du fichier");

    let content = fs::read_to_string(file_path(&mode, "t-1")).expect("lecture du fichier");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("JSON valide");
    let array = parsed.as_array().expect("tableau racine, sans objet englobant");
    assert_eq!(array.len(), 2);
    assert_eq!(
        array[0].as_object().expect("objet de passage").len(),
        6,
        "exactement les six clés de la spécification"
    );
    for key in [
        "commune",
        "passage",
        "nb_passages_total",
        "km_entree",
        "km_sortie",
        "longueur_km",
    ] {
        assert!(array[0].get(key).is_some(), "clé manquante : {key}");
    }
    assert!(content.contains("\n  {"), "indentation de 2 espaces");

    // Aller-retour par le type du module : le fichier est relisible tel quel.
    let reloaded: Vec<CommunePassage> =
        serde_json::from_str(&content).expect("relecture par le type du module");
    assert_eq!(reloaded, passages);

    let _ = fs::remove_dir_all(&mode);
}
