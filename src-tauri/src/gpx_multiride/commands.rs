//! Commandes Tauri du module Multiride.
//!
//! Les commandes publiques, appelées par le store Pinia `useMultirideStore` :
//! détection et relecture (`multiride_detect`, `multiride_load`), ajustements
//! (`multiride_merge_segment`, `multiride_toggle_fp`, `multiride_reset`,
//! `multiride_undo_segment`) et validation (`multiride_validate`). Aucune
//! n'entretient d'état côté Rust : le fichier de description
//! (`traces/{trace_id}/multiride.json`) est la seule persistance, et il est
//! réécrit par chaque commande qui modifie l'état.
//!
//! Chaque commande est un **adaptateur mince** : la logique réside dans une
//! fonction interne (`detect_impl`, `validate_impl`, …) qui ne dépend pas de
//! Tauri et reste donc testable directement — même découpage que
//! `gpx_audit::commands`.
//!
//! Le module expose en outre le **point d'entrée des chaînes automatiques**
//! (`detect_status`), appelé par l'import d'une trace déjà valide et par la
//! validation d'un audit : la détection des passages multiples suit l'audit dans
//! le parcours d'une trace, et doit donc se déclencher sans que l'utilisateur
//! ouvre la vue.

use std::path::Path;
use std::sync::{Arc, RwLock};

use tauri::Manager;

use crate::gpx_audit::export::iso_now;
use crate::import_gpx::{
    get_mode_dir, get_trace_gpx_path, get_traces_path, load_registry, save_registry,
};
use crate::settings::{get_toml_value_by_path, SettingsState};

use super::adjustments;
use super::detection;
use super::file;
use super::types::{MultirideArchive, MultirideDetectionResult, MultirideParams};

// ─── Lecture des paramètres ───────────────────────────────────────────

/// Lit les paramètres du détecteur (`Multiride.Detection`) avec repli sur les
/// valeurs par défaut de la spécification.
///
/// Ne **panique jamais** (`try_state`) : les chaînes automatiques (import,
/// validation d'audit) s'appuient dessus, une défaillance du système de
/// paramètres ne doit pas les bloquer.
pub fn read_multiride_params(app: &tauri::AppHandle) -> MultirideParams {
    let state = match app.try_state::<Arc<RwLock<SettingsState>>>() {
        Some(s) => s,
        None => return default_multiride_params(),
    };
    let guard = match state.read() {
        Ok(g) => g,
        Err(_) => return default_multiride_params(),
    };
    multiride_params_from_settings(&guard.default_toml, &guard.user_overrides)
}

/// Valeurs par défaut des paramètres de détection — miroir de la section
/// `Multiride.Detection` de `settings.default.toml` (§6 de la spécification).
pub fn default_multiride_params() -> MultirideParams {
    MultirideParams {
        tolerance_m: 10.0,
        longueur_min_m: 100.0,
        pas_echantillonnage_m: 4.0,
        fusion_references_m: 100.0,
    }
}

/// Résout les paramètres depuis les tables TOML : surcharge utilisateur
/// prioritaire, puis valeur par défaut du schéma, puis repli. Pure et testable.
pub fn multiride_params_from_settings(
    default_toml: &toml::Table,
    user_overrides: &toml::Table,
) -> MultirideParams {
    let fallback = default_multiride_params();
    let number = |path: &str, default: f64| -> f64 {
        let value = get_toml_value_by_path(user_overrides, path)
            .or_else(|| get_toml_value_by_path(default_toml, path));
        match value {
            Some(toml::Value::Float(v)) => *v,
            Some(toml::Value::Integer(v)) => *v as f64,
            _ => default,
        }
    };

    MultirideParams {
        tolerance_m: number("Multiride.Detection.tolerance", fallback.tolerance_m),
        longueur_min_m: number("Multiride.Detection.longueurMin", fallback.longueur_min_m),
        pas_echantillonnage_m: number(
            "Multiride.Detection.pasEchantillonnage",
            fallback.pas_echantillonnage_m,
        ),
        fusion_references_m: number(
            "Multiride.Detection.fusionReferences",
            fallback.fusion_references_m,
        ),
    }
}

// ─── Détection ────────────────────────────────────────────────────────

/// Joue la détection et écrit le fichier de description, **sans toucher au
/// registre** : l'appelant décide du statut à y poser et l'écrit dans la même
/// passe que ses propres modifications (import d'une trace, validation d'audit).
pub fn detect_and_save(
    mode_dir: &Path,
    trace_id: &str,
    source: &str,
    gpx_path: &Path,
    params: MultirideParams,
) -> Result<MultirideArchive, String> {
    let archive = detection::detect(trace_id, source, gpx_path, params)?;
    file::save_file(&file::file_path(mode_dir, trace_id), &archive)?;
    Ok(archive)
}

/// Détection d'une **chaîne automatique** — import d'une trace déjà valide,
/// validation d'audit — : retourne le statut à poser dans le registre, ou `None`
/// si la détection n'a pas pu aboutir.
///
/// Best-effort assumé : ces chaînes ne doivent jamais échouer à cause de la
/// détection des passages multiples. Un échec — **y compris un panic**, la
/// détection manipulant des indices calculés — laisse la trace non détectée,
/// donc permissive : au pire l'étape des passages multiples reste à faire, jamais
/// l'opération déclenchante n'est bloquée. La détection lancée depuis la vue,
/// elle, remonte ses erreurs à l'utilisateur.
pub fn detect_status(
    mode_dir: &Path,
    trace_id: &str,
    source: &str,
    gpx_path: &Path,
    params: MultirideParams,
) -> Option<String> {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        detect_and_save(mode_dir, trace_id, source, gpx_path, params)
    }));

    match outcome {
        Ok(Ok(archive)) => Some(archive.status().to_string()),
        Ok(Err(error)) => {
            eprintln!(
                "[multiride] détection non jouée pour {} : {}",
                trace_id, error
            );
            None
        }
        Err(_) => {
            eprintln!(
                "[multiride] détection interrompue pour {} : panique",
                trace_id
            );
            None
        }
    }
}

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

    let archive = detect_and_save(mode_dir, trace_id, &trace.filename, &gpx_path, params)?;
    let status = set_status(mode_dir, trace_id, archive.status())?;

    Ok(MultirideDetectionResult {
        archive,
        status,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

// ─── Ajustements ──────────────────────────────────────────────────────

/// Fusionne un segment avec le précédent (spécification §F-14) et réécrit le
/// fichier de description.
///
/// Le **registre n'est pas touché** : un ajustement n'a pas d'incidence sur
/// l'édition caméra, le statut de la trace reste donc celui de la détection ou
/// de la validation.
#[tauri::command]
pub async fn multiride_merge_segment(
    app: tauri::AppHandle,
    trace_id: String,
    archive: MultirideArchive,
    segment: usize,
) -> Result<MultirideArchive, String> {
    let mode_dir = get_mode_dir(&app)?;
    let updated = merge_impl(&mode_dir, &trace_id, archive, segment)?;
    println!(
        "[multiride] fusion trace={} segment={} passages={}",
        trace_id,
        segment,
        updated.passages.len()
    );
    Ok(updated)
}

/// Marque ou démarque un segment en faux positif (spécification §F-15) et
/// réécrit le fichier de description.
#[tauri::command]
pub async fn multiride_toggle_fp(
    app: tauri::AppHandle,
    trace_id: String,
    archive: MultirideArchive,
    segment: usize,
) -> Result<MultirideArchive, String> {
    let mode_dir = get_mode_dir(&app)?;
    let updated = toggle_fp_impl(&mode_dir, &trace_id, archive, segment)?;
    println!(
        "[multiride] faux positif trace={} segment={} écarté={}",
        trace_id,
        segment,
        updated
            .passages
            .iter()
            .filter(|p| p.segment == segment)
            .all(|p| p.faux_positif)
    );
    Ok(updated)
}

/// Annule l'ajustement d'un segment (spécification §F-14, §F-15) et réécrit le
/// fichier de description.
///
/// Un segment ne portant qu'un ajustement à la fois, la commande n'a pas à
/// savoir lequel elle annule : l'état reçu le dit — marqueur faux positif à
/// retirer, ou enregistrement d'avant fusion à réinstaller.
///
/// Le **registre n'est pas touché**, comme pour les autres ajustements.
#[tauri::command]
pub async fn multiride_undo_segment(
    app: tauri::AppHandle,
    trace_id: String,
    archive: MultirideArchive,
    segment: usize,
) -> Result<MultirideArchive, String> {
    let mode_dir = get_mode_dir(&app)?;
    let updated = undo_impl(&mode_dir, &trace_id, archive, segment)?;
    println!(
        "[multiride] annulation trace={} segment={} passages={}",
        trace_id,
        segment,
        updated.passages.len()
    );
    Ok(updated)
}

/// Rétablit la détection d'origine en la rejouant, et réécrit le fichier de
/// description (spécification §F-16).
#[tauri::command]
pub async fn multiride_reset(
    app: tauri::AppHandle,
    trace_id: String,
    archive: MultirideArchive,
) -> Result<MultirideArchive, String> {
    let mode_dir = get_mode_dir(&app)?;
    let updated = reset_impl(&mode_dir, &trace_id, archive)?;
    println!(
        "[multiride] réinitialisation trace={} passages={}",
        trace_id,
        updated.passages.len()
    );
    Ok(updated)
}

/// Implémentation testable de `multiride_merge_segment` (sans `AppHandle`).
pub fn merge_impl(
    mode_dir: &Path,
    trace_id: &str,
    archive: MultirideArchive,
    segment: usize,
) -> Result<MultirideArchive, String> {
    let updated = adjustments::merge_segment(&archive, segment)?;
    save_adjusted(mode_dir, trace_id, &updated)?;
    Ok(updated)
}

/// Implémentation testable de `multiride_toggle_fp` (sans `AppHandle`).
pub fn toggle_fp_impl(
    mode_dir: &Path,
    trace_id: &str,
    archive: MultirideArchive,
    segment: usize,
) -> Result<MultirideArchive, String> {
    let updated = adjustments::toggle_fp(&archive, segment)?;
    save_adjusted(mode_dir, trace_id, &updated)?;
    Ok(updated)
}

/// Implémentation testable de `multiride_undo_segment` (sans `AppHandle`).
pub fn undo_impl(
    mode_dir: &Path,
    trace_id: &str,
    archive: MultirideArchive,
    segment: usize,
) -> Result<MultirideArchive, String> {
    let updated = adjustments::undo_segment(&archive, segment)?;
    save_adjusted(mode_dir, trace_id, &updated)?;
    Ok(updated)
}

/// Implémentation testable de `multiride_reset` (sans `AppHandle`).
///
/// La réinitialisation **rejoue la détection** avec les paramètres enregistrés
/// dans l'état, plutôt que de conserver une copie de la détection initiale : la
/// détection est déterministe, donc le résultat est le même, et le fichier ne
/// porte pas de baseline redondante. Le statut de validation est conservé — un
/// ajustement n'a pas d'incidence sur l'édition caméra.
pub fn reset_impl(
    mode_dir: &Path,
    trace_id: &str,
    archive: MultirideArchive,
) -> Result<MultirideArchive, String> {
    if archive.trace_id != trace_id {
        return Err(format!(
            "L'état à réinitialiser appartient à une autre trace ({} au lieu de {}).",
            archive.trace_id, trace_id
        ));
    }

    let gpx_path = get_trace_gpx_path(mode_dir, trace_id, &archive.source);
    let detected = detection::detect(trace_id, &archive.source, &gpx_path, archive.params.clone())?;
    let fresh = MultirideArchive {
        valide: archive.valide,
        ..detected
    };
    file::save_file(&file::file_path(mode_dir, trace_id), &fresh)?;
    Ok(fresh)
}

/// Écrit l'état ajusté, après vérification de son rattachement à la trace.
fn save_adjusted(
    mode_dir: &Path,
    trace_id: &str,
    archive: &MultirideArchive,
) -> Result<(), String> {
    if archive.trace_id != trace_id {
        return Err(format!(
            "L'état ajusté appartient à une autre trace ({} au lieu de {}).",
            archive.trace_id, trace_id
        ));
    }
    file::save_file(&file::file_path(mode_dir, trace_id), archive)
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
