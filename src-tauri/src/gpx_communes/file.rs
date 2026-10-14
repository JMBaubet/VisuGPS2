//! Accès disque du module Communes : le fichier `communes.json` d'une trace.
//!
//! Un seul fichier par trace, dans son dossier — même contrat que les archives
//! des modules Audit (`audit.json`) et Multiride (`multiride.json`).

use std::path::{Path, PathBuf};

/// Nom du fichier de description des communes, dans le dossier de la trace.
const FILE_NAME: &str = "communes.json";

/// Chemin du fichier d'une trace : `{mode}/traces/{trace_id}/communes.json`.
pub fn file_path(mode_dir: &Path, trace_id: &str) -> PathBuf {
    mode_dir.join("traces").join(trace_id).join(FILE_NAME)
}

/// Supprime le fichier `communes.json` d'une trace, s'il existe.
///
/// Appelé par `audit_validate` : la réécriture du GPX change la géométrie de la
/// trace, la description des communes ne décrit donc plus cette trace.
/// Tolérant et silencieux à l'usage : l'absence du fichier n'est pas une
/// erreur — c'est le cas courant d'une trace jamais détectée.
pub fn remove_file(mode_dir: &Path, trace_id: &str) {
    let path = file_path(mode_dir, trace_id);
    if !path.exists() {
        return;
    }
    if let Err(e) = std::fs::remove_file(&path) {
        // Non bloquant : le drapeau `communes_completed` est reposé à `false`
        // par l'appelant, la description périmée sera réécrite par la
        // prochaine détection.
        eprintln!("[communes] Suppression de {:?} impossible : {}", path, e);
    }
}
