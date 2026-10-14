//! Commandes Tauri du module Communes.
//!
//! Une seule commande, `communes_detect` : elle reçoit la **réponse Overpass
//! brute** et fait le reste côté backend — lecture de la trace nettoyée,
//! algorithme, écriture du fichier de description, drapeau du registre. Le
//! réseau reste donc côté front (WebView), comme la clé OpenRouteService et les
//! tuiles Mapbox ; le backend se charge de la géométrie et du disque.
//!
//! L'adaptateur Tauri est **mince** : la logique réside dans `detect_impl`, qui
//! ne dépend pas de Tauri et reste testable directement — même approche que
//! `gpx_audit::commands` et `gpx_multiride::commands`.

use std::path::Path;
use std::sync::{Arc, RwLock};

use tauri::Manager;

use crate::import_gpx::{
    get_geojson_path, get_mode_dir, get_traces_path, load_registry, rebuild_geometry_from_gpx,
    save_registry,
};
use crate::settings::{get_toml_value_by_path, SettingsState};

use super::file;
use super::osm;
use super::pipeline;
use super::types::{CommuneParams, CommunePassage};

// ─── Lecture des paramètres ───────────────────────────────────────────

/// Valeurs par défaut des paramètres de détection (miroir de
/// `settings.default.toml`).
pub fn default_communes_params() -> CommuneParams {
    CommuneParams::default()
}

/// Lit les paramètres `Communes.Detection.*` (surcharge utilisateur prioritaire,
/// puis valeur par défaut, puis repli). Ne **panique jamais** (`try_state`) :
/// une défaillance du système de paramètres ne doit pas empêcher une détection.
pub fn read_communes_params(app: &tauri::AppHandle) -> CommuneParams {
    let state = match app.try_state::<Arc<RwLock<SettingsState>>>() {
        Some(s) => s,
        None => return default_communes_params(),
    };
    let guard = match state.read() {
        Ok(g) => g,
        Err(_) => return default_communes_params(),
    };
    communes_params_from_settings(&guard.default_toml, &guard.user_overrides)
}

/// Résout les paramètres depuis les tables TOML (surcharge prioritaire, puis
/// défaut, puis repli). Pure et testable.
pub fn communes_params_from_settings(
    default_toml: &toml::Table,
    user_overrides: &toml::Table,
) -> CommuneParams {
    let fallback = default_communes_params();

    let f = |path: &str, def: f64| -> f64 {
        let value = get_toml_value_by_path(user_overrides, path)
            .or_else(|| get_toml_value_by_path(default_toml, path));
        match value {
            Some(toml::Value::Float(v)) => *v,
            Some(toml::Value::Integer(v)) => *v as f64,
            _ => def,
        }
    };
    let u = |path: &str, def: usize| -> usize {
        let value = get_toml_value_by_path(user_overrides, path)
            .or_else(|| get_toml_value_by_path(default_toml, path));
        match value {
            Some(toml::Value::Integer(v)) if *v > 0 => *v as usize,
            _ => def,
        }
    };
    let s = |path: &str, def: &str| -> String {
        let value = get_toml_value_by_path(user_overrides, path)
            .or_else(|| get_toml_value_by_path(default_toml, path));
        match value {
            Some(toml::Value::String(v)) if !v.trim().is_empty() => v.clone(),
            _ => def.to_string(),
        }
    };

    CommuneParams {
        admin_level: s("Communes.Detection.adminLevel", &fallback.admin_level),
        bbox_buffer_deg: f("Communes.Detection.bboxBufferDeg", fallback.bbox_buffer_deg),
        simplify_tolerance: f(
            "Communes.Detection.simplifyTolerance",
            fallback.simplify_tolerance,
        ),
        target_samples: u("Communes.Detection.targetSamples", fallback.target_samples),
        min_sample_step_km: f(
            "Communes.Detection.minSampleStepKm",
            fallback.min_sample_step_km,
        ),
        max_area_ratio: f("Communes.Detection.maxAreaRatio", fallback.max_area_ratio),
        min_segment_km: f("Communes.Detection.minSegmentKm", fallback.min_segment_km),
        min_keep_km: f("Communes.Detection.minKeepKm", fallback.min_keep_km),
    }
}

// ─── Détection ────────────────────────────────────────────────────────

/// Détecte les communes traversées par une trace et persiste le résultat.
///
/// `osm` porte la **réponse Overpass brute** (tableau des `elements`, sérialisé
/// en JSON par le front) : le front ne transmet jamais la géométrie de la trace,
/// le backend la relit depuis son propre stockage.
///
/// Écrit `traces/{trace_id}/communes.json`, puis pose `communes_completed` dans
/// le registre. Retourne les passages écrits.
#[tauri::command]
pub async fn communes_detect(
    app: tauri::AppHandle,
    trace_id: String,
    osm: String,
) -> Result<Vec<CommunePassage>, String> {
    let mode_dir = get_mode_dir(&app)?;
    let params = read_communes_params(&app);
    detect_impl(&mode_dir, &trace_id, &osm, &params)
}

/// Implémentation testable de `communes_detect`.
///
/// Le drapeau du registre est posé en **dernière** opération : un échec en amont
/// (trace absente, réponse OSM illisible, écriture impossible) laisse la
/// détection à faire pour cette trace — reprise au lancement suivant, ou au clic
/// sur la carte du circuit.
pub fn detect_impl(
    mode_dir: &Path,
    trace_id: &str,
    osm_json: &str,
    params: &CommuneParams,
) -> Result<Vec<CommunePassage>, String> {
    // 1. Trace nettoyée : sa géométrie persistée, régénérée depuis le GPX si elle
    //    manque (trace importée avant ce cache).
    let trace = load_trace_coordinates(mode_dir, trace_id)?;

    // 2. Communes reçues d'Overpass.
    let (communes, stats) = osm::parse_communes(osm_json, &params.admin_level)?;
    println!(
        "[communes] {} : {} élément(s) OSM, {} relation(s) retenue(s), {} anneau(x), {} refermé(s) de force, {} écarté(s).",
        trace_id,
        stats.elements,
        stats.relations,
        stats.rings,
        stats.forced_closures,
        stats.orphan_rings
    );

    // 3. Détection.
    let passages = pipeline::detect_passages(&trace, &communes, params)?;

    // 4. Fichier de description (écriture atomique).
    file::save_file(mode_dir, trace_id, &passages)?;

    // 5. Registre : la détection est réalisée pour cette trace.
    let traces_path = get_traces_path(mode_dir);
    let mut registry = load_registry(&traces_path);
    let entry = registry
        .iter_mut()
        .find(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable : {}.", trace_id))?;
    entry.communes_completed = true;
    save_registry(&traces_path, &registry)?;

    let distinct = passages
        .iter()
        .map(|p| p.commune.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len();
    println!(
        "[communes] {} : {} passage(s) dans {} commune(s).",
        trace_id,
        passages.len(),
        distinct
    );

    Ok(passages)
}

// ─── Consultation et réinitialisation ─────────────────────────────────

/// Charge la description des communes d'une trace (consultation).
///
/// `None` si le fichier est absent ou illisible : la carte du circuit ne
/// propose la consultation que pour une trace détectée, cet état ne devrait
/// donc pas se produire — il n'est pas une erreur pour autant.
#[tauri::command]
pub async fn communes_load(
    app: tauri::AppHandle,
    trace_id: String,
) -> Result<Option<Vec<CommunePassage>>, String> {
    let mode_dir = get_mode_dir(&app)?;
    Ok(file::load_file(&mode_dir, &trace_id))
}

/// Supprime la description des communes d'une trace et repose son drapeau à
/// `false`.
///
/// Outil de **diagnostic**, exposé dans la fenêtre de consultation en mode
/// debug : la trace redevient « à détecter », elle sera reprise au prochain
/// lancement ou au clic sur la carte du circuit.
#[tauri::command]
pub async fn communes_reset(app: tauri::AppHandle, trace_id: String) -> Result<(), String> {
    let mode_dir = get_mode_dir(&app)?;
    reset_impl(&mode_dir, &trace_id)
}

/// Implémentation testable de `communes_reset`.
///
/// La trace est d'abord cherchée au registre — une trace inconnue ne change
/// rien —, puis le fichier est supprimé et le drapeau reposé dans la même
/// passe : l'ordre inverse laisserait une trace « détectée » sans description.
pub fn reset_impl(mode_dir: &Path, trace_id: &str) -> Result<(), String> {
    let traces_path = get_traces_path(mode_dir);
    let mut registry = load_registry(&traces_path);
    if !registry.iter().any(|t| t.id == trace_id) {
        return Err(format!("Trace introuvable : {}.", trace_id));
    }

    file::remove_file(mode_dir, trace_id);

    let entry = registry
        .iter_mut()
        .find(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable : {}.", trace_id))?;
    entry.communes_completed = false;
    save_registry(&traces_path, &registry)?;

    println!("[communes] {} : détection réinitialisée.", trace_id);
    Ok(())
}

/// Coordonnées `[lon, lat]` de la trace persistée (`trace.geojson`).
fn load_trace_coordinates(mode_dir: &Path, trace_id: &str) -> Result<Vec<[f64; 2]>, String> {
    let geojson_path = get_geojson_path(mode_dir, trace_id);
    let geometry = if geojson_path.exists() {
        let content = std::fs::read_to_string(&geojson_path)
            .map_err(|e| format!("Lecture de la géométrie de trace : {}", e))?;
        serde_json::from_str::<serde_json::Value>(&content)
            .map_err(|e| format!("Géométrie de trace illisible : {}", e))?
    } else {
        let registry = load_registry(&get_traces_path(mode_dir));
        rebuild_geometry_from_gpx(mode_dir, &registry, trace_id)?
    };
    coordinates_of(&geometry)
}

/// Extrait les coordonnées d'une Feature GeoJSON LineString.
fn coordinates_of(geometry: &serde_json::Value) -> Result<Vec<[f64; 2]>, String> {
    let raw = geometry
        .get("geometry")
        .and_then(|g| g.get("coordinates"))
        .and_then(|c| c.as_array())
        .ok_or_else(|| "Géométrie de trace illisible (LineString attendu).".to_string())?;

    raw.iter()
        .map(|point| {
            let unreadable = || "Coordonnée de trace illisible.".to_string();
            let pair = point.as_array().filter(|p| p.len() >= 2).ok_or_else(unreadable)?;
            let lon = pair[0].as_f64().ok_or_else(unreadable)?;
            let lat = pair[1].as_f64().ok_or_else(unreadable)?;
            Ok([lon, lat])
        })
        .collect()
}
