//! Commandes Tauri du module Multiride.
//!
//! Trois commandes publiques, appelées par le store Pinia `useMultirideStore`.
//! Aucune n'entretient d'état côté Rust : le fichier de description
//! (`traces/{trace_id}/multiride.json`) est la seule persistance, et il est
//! réécrit par chaque commande qui modifie l'état.
//!
//! Chaque commande est un **adaptateur mince** : la logique réside dans une
//! fonction interne (`detect_impl`, `validate_impl`, …) qui ne dépend pas de
//! Tauri et reste donc testable directement — même découpage que
//! `gpx_audit::commands`.

use std::path::Path;

use crate::gpx_audit::export::iso_now;
use crate::import_gpx::{
    get_mode_dir, get_trace_gpx_path, get_traces_path, load_registry, save_registry,
};

use super::detection;
use super::file;
use super::types::{MultirideArchive, MultirideDetectionResult, MultirideParams};

// ─── Détection ────────────────────────────────────────────────────────

/// Détecte les passages multiples d'une trace, écrit le fichier de description
/// et pose le statut de la trace (`none` ou `pending`).
#[tauri::command]
pub async fn multiride_detect(
    app: tauri::AppHandle,
    trace_id: String,
    params: MultirideParams,
) -> Result<MultirideDetectionResult, String> {
    let mode_dir = get_mode_dir(&app)?;
    let result = detect_impl(&mode_dir, &trace_id, params)?;
    println!(
        "[multiride] detection trace={} passages={} statut={} durée={}ms",
        trace_id,
        result.archive.passages.len(),
        result.status,
        result.duration_ms
    );
    Ok(result)
}

/// Implémentation testable de `multiride_detect` (sans `AppHandle`).
///
/// La trace est désignée par son identifiant : son GPX est relu pour mesurer la
/// trace d'origine, puis la détection produit l'état, qui est écrit et résumé
/// par le statut du registre.
pub fn detect_impl(
    mode_dir: &Path,
    trace_id: &str,
    params: MultirideParams,
) -> Result<MultirideDetectionResult, String> {
    let started = std::time::Instant::now();

    let traces_path = get_traces_path(mode_dir);
    let registry = load_registry(&traces_path);
    let trace = registry
        .iter()
        .find(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable : {}.", trace_id))?;
    let gpx_path = get_trace_gpx_path(mode_dir, trace_id, &trace.filename);

    let archive = detection::detect(trace_id, &trace.filename, &gpx_path, params)?;
    file::save_file(&file::file_path(mode_dir, trace_id), &archive)?;
    let status = set_status(mode_dir, trace_id, archive.status())?;

    Ok(MultirideDetectionResult {
        archive,
        status,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

// ─── Relecture ────────────────────────────────────────────────────────

/// Relit le fichier de description d'une trace (`null` si absent ou
/// inexploitable — l'appelant relance alors la détection).
#[tauri::command]
pub async fn multiride_load(
    app: tauri::AppHandle,
    trace_id: String,
) -> Result<Option<MultirideArchive>, String> {
    let mode_dir = get_mode_dir(&app)?;
    Ok(load_impl(&mode_dir, &trace_id))
}

/// Implémentation testable de `multiride_load` (sans `AppHandle`).
pub fn load_impl(mode_dir: &Path, trace_id: &str) -> Option<MultirideArchive> {
    file::load_file(&file::file_path(mode_dir, trace_id), trace_id)
}

// ─── Validation ───────────────────────────────────────────────────────

/// Valide les passages multiples : marque l'état `valide`, réécrit le fichier
/// de description et lève la barrière de l'édition caméra.
///
/// Les ajustements ultérieurs (fusion, faux positif) restent possibles et
/// réécrivent le fichier : ils n'ont pas d'incidence sur l'édition caméra, et
/// le statut ne rebascule pas.
#[tauri::command]
pub async fn multiride_validate(
    app: tauri::AppHandle,
    trace_id: String,
    archive: MultirideArchive,
) -> Result<MultirideArchive, String> {
    let mode_dir = get_mode_dir(&app)?;
    let updated = validate_impl(&mode_dir, &trace_id, archive)?;
    println!(
        "[multiride] validate trace={} passages={} statut={}",
        trace_id,
        updated.passages.len(),
        updated.status()
    );
    Ok(updated)
}

/// Implémentation testable de `multiride_validate` (sans `AppHandle`).
pub fn validate_impl(
    mode_dir: &Path,
    trace_id: &str,
    mut archive: MultirideArchive,
) -> Result<MultirideArchive, String> {
    if archive.trace_id != trace_id {
        return Err(format!(
            "L'état à valider appartient à une autre trace ({} au lieu de {}).",
            archive.trace_id, trace_id
        ));
    }

    archive.valide = true;
    archive.updated_at = iso_now();
    file::save_file(&file::file_path(mode_dir, trace_id), &archive)?;
    set_status(mode_dir, trace_id, archive.status())?;

    Ok(archive)
}

// ─── Registre ─────────────────────────────────────────────────────────

/// Pose `multiride_status` dans le registre des traces et retourne le statut
/// écrit (registre réécrit de façon atomique, comme partout ailleurs).
///
/// Une trace absente du registre est une incohérence d'appel, pas une panne :
/// elle est remontée à l'appelant.
fn set_status(mode_dir: &Path, trace_id: &str, status: &str) -> Result<String, String> {
    let traces_path = get_traces_path(mode_dir);
    let mut registry = load_registry(&traces_path);
    let entry = registry
        .iter_mut()
        .find(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable : {}.", trace_id))?;
    entry.multiride_status = Some(status.to_string());
    save_registry(&traces_path, &registry)?;
    Ok(status.to_string())
}
