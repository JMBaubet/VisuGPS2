//! Migration vers le module Audit (plan de migration, livrable 5 §3 et §4).
//!
//! Deux mécanismes, tous deux **silencieux** (D3b — aucun log, aucune erreur
//! remontée à l'utilisateur) :
//! - **D1** : détection d'un registre `traces.json` au format pré-audit
//!   (champ `cleaning_status`) — le registre est ignoré, **jamais écrasé** ;
//! - **D2c** : suppression des fichiers de travail orphelins de l'ancien
//!   module de nettoyage (`cleaning.*.json`).

use std::fs;
use std::path::Path;

/// D1 — registre obsolète (format pré-audit).
///
/// Le test porte sur la **sous-chaîne quoteé** `"cleaning_status"` : le champ
/// ne peut apparaître qu'en clé JSON, jamais isolé dans une valeur.
pub fn is_obsolete_registry(raw: &str) -> bool {
    raw.contains("\"cleaning_status\"")
}

/// D2c — supprime les fichiers `cleaning.*.json` orphelins de tous les dossiers
/// de traces du mode.
///
/// Idempotent (aucun drapeau à maintenir) et silencieux (D3b) : un dossier
/// absent, une entrée illisible ou une suppression impossible ne produisent ni
/// log ni erreur — la migration ne doit jamais faire échouer le démarrage.
pub fn cleanup_obsolete_cleaning_files(mode_dir: &Path) {
    let traces_dir = mode_dir.join("traces");
    if !traces_dir.exists() {
        return;
    }

    let Ok(entries) = fs::read_dir(&traces_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let trace_dir = entry.path();
        if !trace_dir.is_dir() {
            continue;
        }
        let Ok(files) = fs::read_dir(&trace_dir) else {
            continue;
        };
        for f in files.flatten() {
            let path = f.path();
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                if name.starts_with("cleaning.") && name.ends_with(".json") {
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }
}
