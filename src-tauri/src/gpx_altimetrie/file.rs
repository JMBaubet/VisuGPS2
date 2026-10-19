//! Fichier `traces/{trace_id}/altimetrie.json`.
//!
//! Deux garanties, reprises de `gpx_audit::archive` et `gpx_multiride::file` :
//! - l'écriture est **atomique** (`pipeline::write_atomic`, `.tmp` + `rename`) :
//!   un échec laisse le fichier précédent intact ;
//! - la lecture est **tolérante** : fichier absent, illisible, d'une version
//!   inconnue ou rattaché à une autre trace → `None`. L'appelant relance alors
//!   la détection, qui reste la source de vérité.

use std::path::{Path, PathBuf};

use crate::gpx_audit::pipeline::write_atomic;

use super::types::{AltimetrieFile, FILE_VERSION};

/// Nom du fichier, dans le dossier de la trace.
pub const FILE_NAME: &str = "altimetrie.json";

/// Chemin du fichier d'une trace : `{mode}/traces/{trace_id}/altimetrie.json`.
pub fn file_path(mode_dir: &Path, trace_id: &str) -> PathBuf {
    mode_dir
        .join("traces")
        .join(trace_id)
        .join(FILE_NAME)
}

/// Écrit le fichier d'analyse (écriture atomique). Le dossier de la trace est
/// créé au besoin.
pub fn save_file(path: &Path, file: &AltimetrieFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Création du dossier du fichier d'altimétrie ({}): {}",
                parent.display(),
                e
            )
        })?;
    }
    let json = serde_json::to_string_pretty(file)
        .map_err(|e| format!("Sérialisation de l'altimétrie : {}", e))?;
    write_atomic(path, json.as_bytes())
}

/// Lit le fichier d'analyse d'une trace.
///
/// Retourne `None` si le fichier est absent, illisible, d'une version inconnue
/// ou rattaché à une autre trace : l'appelant relance la détection.
pub fn load_file(path: &Path, trace_id: &str) -> Option<AltimetrieFile> {
    let content = std::fs::read_to_string(path).ok()?;
    let file: AltimetrieFile = serde_json::from_str(&content).ok()?;
    if file.version != FILE_VERSION || file.trace_id != trace_id {
        return None;
    }
    Some(file)
}

/// Supprime le fichier d'analyse d'une trace. Tolérant si le fichier est
/// absent ; une autre erreur est loggée sans être propagée (la suppression est
/// une invalidation best-effort avant régénération).
pub fn remove_file(mode_dir: &Path, trace_id: &str) {
    let path = file_path(mode_dir, trace_id);
    if let Err(error) = std::fs::remove_file(&path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            eprintln!(
                "[altimetrie] suppression impossible ({}): {}",
                path.display(),
                error
            );
        }
    }
}
