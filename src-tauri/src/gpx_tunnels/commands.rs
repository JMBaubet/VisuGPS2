//! Commandes Tauri du module Tunnels.
//!
//! - `tunnels_load` : relit `tunnels.json` ;
//! - `tunnels_requirements` : décrit les ressources OSM nécessaires à la trace ;
//! - `tunnels_prepare` : télécharge silencieusement `regions.json` et les PBF
//!   manquants, puis détecte ;
//! - `tunnels_detect` : détecte sans téléchargement (relance quand les
//!   ressources sont présentes) ;
//! - `tunnels_save` : valide les bornes, recalcule les longueurs, écrit le
//!   fichier et pose `success` ;
//! - `tunnels_support_status` : état global du support OSM ;
//! - `pick_directory` : sélecteur natif du dossier OSM.
//!
//! Le fichier `traces/{trace_id}/tunnels.json` est la seule persistance dans le
//! dossier de la trace. `regions.json` et les `.osm.pbf` vivent sur le support
//! OSM. Les téléchargements sont silencieux ; la progression est émise par
//! l'événement `tunnels-progress`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use tauri::Emitter;
use tauri_plugin_dialog::DialogExt;

use crate::gpx_audit::export::iso_now;
use crate::import_gpx::{
    get_mode_dir, get_trace_gpx_path, get_traces_path, load_registry, update_registry,
};

use super::download;
use super::file;
use super::geometry;
use super::params::read_config;
use super::pbf;
use super::pipeline;
use super::regions;
use super::types::{
    PbfFileRef, Tunnel, TunnelFile, TunnelParams, TunnelPrepareResult, TunnelProgress,
    TunnelRequirements, TunnelSupportStatus, FILE_VERSION, STATUS_ERROR,
    STATUS_MISSING_PBF, STATUS_NOT_CONFIGURED, STATUS_SUCCESS, STATUS_WAITING_VALIDATE,
};

/// Une seule préparation à la fois. Le verrou tokio est `Send` et peut être tenu
/// pendant les téléchargements, contrairement à un `std::sync::MutexGuard`.
static PREPARE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const PROGRESS_EVENT: &str = "tunnels-progress";
const MIN_TUNNEL_LENGTH_KM: f64 = 0.01;

// ─── Résolution et helpers ────────────────────────────────────────────

fn resolve_trace(
    mode_dir: &Path,
    trace_id: &str,
) -> Result<(String, PathBuf, String), String> {
    let traces_path = get_traces_path(mode_dir);
    let registry = load_registry(&traces_path);
    let trace = registry
        .iter()
        .find(|trace| trace.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable : {}.", trace_id))?;
    let gpx_path = get_trace_gpx_path(mode_dir, trace_id, &trace.filename);
    Ok((trace.filename.clone(), gpx_path, trace.hash.clone()))
}

fn load_tunnel_file(mode_dir: &Path, trace_id: &str) -> Option<TunnelFile> {
    file::load_file(&file::file_path(mode_dir, trace_id), trace_id)
}

fn read_points_and_bbox(
    gpx_path: &Path,
    margin_m: f64,
) -> Result<(Vec<[f64; 2]>, [f64; 4]), String> {
    let points = pipeline::load_points(gpx_path)?;
    let bbox = geometry::bbox_with_margin(&points, margin_m);
    Ok((points, bbox))
}

fn pbf_ref(path: &Path) -> PbfFileRef {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string();
    let modified_at = std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .map(|time| chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339());
    PbfFileRef { name, modified_at }
}

/// Nombre de tunnels logiques : les membres d'un même `groupId` comptent une
/// fois.
pub fn group_count(tunnels: &[Tunnel]) -> u32 {
    let mut groups: HashSet<&str> = HashSet::new();
    let mut singles = 0_u32;
    for tunnel in tunnels {
        match tunnel.group_id.as_deref() {
            Some(group) => {
                groups.insert(group);
            }
            None => singles += 1,
        }
    }
    groups.len() as u32 + singles
}

fn emit_progress(
    app: &tauri::AppHandle,
    trace_id: &str,
    stage: &str,
    region: Option<&str>,
    filename: Option<&str>,
    bytes_done: u64,
    bytes_total: u64,
) {
    let percent = if bytes_total > 0 {
        (bytes_done as f64 * 100.0 / bytes_total as f64).min(100.0)
    } else {
        0.0
    };
    let progress = TunnelProgress {
        trace_id: trace_id.to_string(),
        stage: stage.to_string(),
        region: region.map(str::to_string),
        filename: filename.map(str::to_string),
        bytes_done,
        bytes_total,
        percent,
    };
    if let Err(error) = app.emit(PROGRESS_EVENT, progress) {
        eprintln!("[tunnels] progression non émise : {}", error);
    }
}

fn set_tunnel_state(
    mode_dir: &Path,
    trace_id: &str,
    status: &str,
    count: Option<u32>,
) {
    let traces_path = get_traces_path(mode_dir);
    let result = update_registry(&traces_path, |registry| {
        if let Some(entry) = registry.iter_mut().find(|trace| trace.id == trace_id) {
            entry.tunnels_status = Some(status.to_string());
            entry.tunnels_count = count;
        }
        Ok(())
    });
    if let Err(error) = result {
        eprintln!("[tunnels] mise à jour du registre impossible : {}", error);
    }
}

fn fail(
    mode_dir: &Path,
    trace_id: &str,
    status: &str,
    message: String,
) -> String {
    eprintln!("[tunnels] {} : {}", status, message);
    set_tunnel_state(mode_dir, trace_id, status, None);
    message
}

/// Rattrapage de `tunnels_status` pour les fichiers existants d'un registre
/// antérieur au champ. Appelé par `get_traces`, idempotent.
pub fn backfill_state(mode_dir: &Path) {
    let traces_path = get_traces_path(mode_dir);
    let registry = load_registry(&traces_path);
    let mut updates: Vec<(String, String, u32)> = Vec::new();

    for entry in &registry {
        if entry.tunnels_status.is_some() {
            continue;
        }
        let Some(file) = load_tunnel_file(mode_dir, &entry.id) else {
            continue;
        };
        let status = if file.tunnels.is_empty() {
            STATUS_SUCCESS
        } else {
            STATUS_WAITING_VALIDATE
        };
        updates.push((
            entry.id.clone(),
            status.to_string(),
            group_count(&file.tunnels),
        ));
    }

    if updates.is_empty() {
        return;
    }
    if let Err(error) = update_registry(&traces_path, |registry| {
        for (id, status, count) in &updates {
            if let Some(entry) = registry.iter_mut().find(|trace| &trace.id == id) {
                entry.tunnels_status = Some(status.clone());
                entry.tunnels_count = Some(*count);
            }
        }
        Ok(())
    }) {
        eprintln!("[tunnels] rattrapage du registre impossible : {}", error);
    }
}

/// Valide et normalise les tunnels reçus du frontend.
///
/// Le backend ne fait jamais confiance aux longueurs dérivées : elles sont
/// recalculées. Les tunnels sont triés et ne peuvent pas se chevaucher. Un
/// groupe fusionné doit contenir au moins deux tronçons.
pub fn normalize_tunnels(
    mut tunnels: Vec<Tunnel>,
    total_km: f64,
) -> Result<Vec<Tunnel>, String> {
    let mut ids: HashSet<String> = HashSet::new();
    let mut groups: HashMap<String, usize> = HashMap::new();

    for tunnel in &mut tunnels {
        if tunnel.id.trim().is_empty() {
            return Err("Tunnel sans identifiant.".to_string());
        }
        if !ids.insert(tunnel.id.clone()) {
            return Err(format!(
                "Identifiant de tunnel dupliqué : {}.",
                tunnel.id
            ));
        }
        if !tunnel.start_km.is_finite() || !tunnel.end_km.is_finite() {
            return Err(format!("Bornes invalides pour le tunnel {}.", tunnel.id));
        }
        if tunnel.start_km < 0.0 {
            return Err(format!(
                "Le tunnel {} commence avant le départ de la trace.",
                tunnel.id
            ));
        }
        if tunnel.end_km > total_km + 1e-6 {
            return Err(format!(
                "Le tunnel {} dépasse la longueur de la trace ({:.3} km).",
                tunnel.id, total_km
            ));
        }
        if tunnel.end_km - tunnel.start_km < MIN_TUNNEL_LENGTH_KM {
            return Err(format!(
                "Le tunnel {} est trop court (minimum 10 m).",
                tunnel.id
            ));
        }

        tunnel.start_km = round_to(tunnel.start_km, 6);
        tunnel.end_km = round_to(tunnel.end_km, 6);
        tunnel.length_m = round_to((tunnel.end_km - tunnel.start_km) * 1000.0, 1);
        if tunnel.detection_method != "osm" && tunnel.detection_method != "manual" {
            tunnel.detection_method = "manual".to_string();
        }
        if let Some(group) = tunnel.group_id.clone() {
            if group.trim().is_empty() {
                tunnel.group_id = None;
            } else {
                *groups.entry(group).or_insert(0) += 1;
            }
        }
    }

    for (group, count) in groups {
        if count < 2 {
            return Err(format!(
                "Le groupe fusionné {} doit contenir au moins deux tunnels.",
                group
            ));
        }
    }

    tunnels.sort_by(|a, b| a.start_km.total_cmp(&b.start_km));
    for pair in tunnels.windows(2) {
        if pair[1].start_km < pair[0].end_km - 1e-9 {
            return Err(format!(
                "Les tunnels {} et {} se chevauchent.",
                pair[0].id, pair[1].id
            ));
        }
    }

    Ok(tunnels)
}

fn round_to(value: f64, decimals: i32) -> f64 {
    let factor = 10_f64.powi(decimals);
    (value * factor).round() / factor
}

// ─── Détection ────────────────────────────────────────────────────────

async fn detect_core(
    app: &tauri::AppHandle,
    mode_dir: &Path,
    trace_id: &str,
    expected_hash: &str,
    params: &TunnelParams,
    pbf_paths: Vec<PathBuf>,
    pbf_refs: Vec<PbfFileRef>,
    points: &[[f64; 2]],
    bbox: [f64; 4],
) -> Result<TunnelPrepareResult, String> {
    let mut all_ways = Vec::new();

    for path in pbf_paths {
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("PBF")
            .to_string();
        emit_progress(app, trace_id, "detect", None, Some(&filename), 0, 0);

        let path_for_parse = path.clone();
        let ways = tauri::async_runtime::spawn_blocking(move || {
            pbf::read_tunnel_ways(&path_for_parse, bbox)
        })
        .await
        .map_err(|error| format!("Analyse PBF interrompue ({}): {}", filename, error))??;
        all_ways.extend(ways);
    }

    let ways = pbf::merge_ways(all_ways);
    let tunnels = pipeline::match_tunnels(points, &ways, params);
    let now = iso_now();
    let file = pipeline::build_file(
        trace_id,
        params,
        pbf_refs,
        tunnels,
        Some(now.clone()),
        &now,
    );

    // Garde de hash : les téléchargements ont pu durer, un audit a pu réécrire
    // le GPX entre-temps. Le fichier décrirait alors l'ancienne géométrie.
    let registry = load_registry(&get_traces_path(mode_dir));
    let current_hash = registry
        .iter()
        .find(|trace| trace.id == trace_id)
        .map(|trace| trace.hash.as_str());
    if current_hash != Some(expected_hash) {
        return Err(
            "La trace a été modifiée pendant la préparation des tunnels ; détection abandonnée."
                .to_string(),
        );
    }

    file::save_file(&file::file_path(mode_dir, trace_id), &file)?;
    let status = if file.tunnels.is_empty() {
        STATUS_SUCCESS
    } else {
        STATUS_WAITING_VALIDATE
    };
    set_tunnel_state(
        mode_dir,
        trace_id,
        status,
        Some(group_count(&file.tunnels)),
    );

    Ok(TunnelPrepareResult {
        file,
        status: status.to_string(),
    })
}

// ─── Commandes publiques ──────────────────────────────────────────────

/// Relit `tunnels.json` d'une trace. `None` si le fichier est absent ou
/// illisible.
#[tauri::command]
pub async fn tunnels_load(
    app: tauri::AppHandle,
    trace_id: String,
) -> Result<Option<TunnelFile>, String> {
    let mode_dir = get_mode_dir(&app)?;
    Ok(load_tunnel_file(&mode_dir, &trace_id))
}

/// Décrit les ressources OSM nécessaires à la détection de la trace.
#[tauri::command]
pub async fn tunnels_requirements(
    app: tauri::AppHandle,
    trace_id: String,
) -> Result<TunnelRequirements, String> {
    let mode_dir = get_mode_dir(&app)?;
    let (_, gpx_path, _) = resolve_trace(&mode_dir, &trace_id)?;
    let (_, support) = read_config(&app).await;
    let points = pipeline::load_points(&gpx_path)?;
    let folder = support.pbf_directory.as_deref().map(PathBuf::from);
    let regions_file = folder.as_deref().and_then(regions::load_regions);

    let mut requirements = regions::build_requirements(
        support.actif,
        folder.as_deref(),
        regions_file.as_ref(),
        &points,
    );

    let max_mb = support.taille_max_pbf_mb;
    if let Some(folder_path) = folder.as_deref() {
        let client = download::client().ok();
        for region in &mut requirements.regions {
            if region.present {
                region.size_mb =
                    regions::pbf_size_mb(folder_path, &region.pbf_filename);
            } else if let Some(client) = &client {
                let url = regions_file
                    .as_ref()
                    .and_then(|file| {
                        file.regions
                            .iter()
                            .find(|entry| entry.id == region.id)
                    })
                    .map(|entry| entry.pbf_url.clone());
                if let Some(url) = url {
                    if let Some(bytes) = download::remote_size(client, &url).await {
                        region.size_mb = Some(
                            ((bytes as f64 / 1_048_576.0) * 10.0).round() / 10.0,
                        );
                    }
                }
            }
            region.too_large = !region.present
                && max_mb > 0
                && region
                    .size_mb
                    .map(|size| size > max_mb as f64)
                    .unwrap_or(false);
        }
    }

    Ok(requirements)
}

/// Prépare les données OSM puis détecte les tunnels d'une trace.
///
/// Silencieux : `regions.json` est produit si nécessaire, les PBF manquants
/// sont téléchargés sans confirmation. La progression est émise par
/// `tunnels-progress`.
#[tauri::command]
pub async fn tunnels_prepare(
    app: tauri::AppHandle,
    trace_id: String,
    ignore_size_limit: Option<bool>,
) -> Result<TunnelPrepareResult, String> {
    let _guard = PREPARE_LOCK
        .try_lock()
        .map_err(|_| "Une préparation de tunnels est déjà en cours.".to_string())?;
    let ignore_size_limit = ignore_size_limit.unwrap_or(false);

    let mode_dir = get_mode_dir(&app)?;
    let (_, gpx_path, expected_hash) = resolve_trace(&mode_dir, &trace_id)?;
    let (params, support) = read_config(&app).await;
    let Some(folder) = support.pbf_directory.as_deref().map(PathBuf::from) else {
        return Err(fail(
            &mode_dir,
            &trace_id,
            STATUS_NOT_CONFIGURED,
            "Aucun dossier OSM n'est configuré.".to_string(),
        ));
    };
    if !support.actif {
        return Err(fail(
            &mode_dir,
            &trace_id,
            STATUS_NOT_CONFIGURED,
            "La détection des tunnels est désactivée.".to_string(),
        ));
    }
    if !folder.is_dir() || std::fs::read_dir(&folder).is_err() {
        return Err(fail(
            &mode_dir,
            &trace_id,
            STATUS_NOT_CONFIGURED,
            format!("Le dossier OSM est inaccessible : {}.", folder.display()),
        ));
    }

    let (points, bbox) =
        read_points_and_bbox(&gpx_path, params.bbox_margin_m)
            .map_err(|error| fail(&mode_dir, &trace_id, STATUS_ERROR, error))?;
    let client = download::client()
        .map_err(|error| fail(&mode_dir, &trace_id, STATUS_ERROR, error))?;

    let app_progress = app.clone();
    let trace_progress = trace_id.clone();
    let regions_file = download::ensure_regions_json(
        &client,
        &folder,
        regions::INDEX_URL,
        move |done, total| {
            emit_progress(
                &app_progress,
                &trace_progress,
                "regions",
                None,
                Some(regions::REGIONS_FILE_NAME),
                done,
                total,
            );
        },
    )
    .await
    .map_err(|error| fail(&mode_dir, &trace_id, STATUS_NOT_CONFIGURED, error))?;

    let required = regions::select_regions(&regions_file, &points);
    if required.is_empty() {
        return Err(fail(
            &mode_dir,
            &trace_id,
            STATUS_ERROR,
            "Aucune région Geofabrik ne couvre la zone de cette trace.".to_string(),
        ));
    }

    for region in &required {
        if regions::pbf_present(&folder, &region.pbf_filename) {
            continue;
        }

        // Garde-fou de taille : une région au-dessus du plafond n'est pas
        // téléchargée silencieusement. L'onglet Tunnels affiche sa taille et
        // propose « Télécharger quand même » (qui relance avec l'option).
        if !ignore_size_limit && support.taille_max_pbf_mb > 0 {
            if let Some(bytes) = download::remote_size(&client, &region.pbf_url).await {
                let max_bytes = support.taille_max_pbf_mb.saturating_mul(1_048_576);
                if bytes > max_bytes {
                    let size_mb = (bytes as f64 / 1_048_576.0).round();
                    return Err(fail(
                        &mode_dir,
                        &trace_id,
                        STATUS_MISSING_PBF,
                        format!(
                            "La région {} ({} Mo) dépasse le plafond de {} Mo. Utilisez « Télécharger quand même » dans l'onglet Tunnels.",
                            region.name, size_mb, support.taille_max_pbf_mb
                        ),
                    ));
                }
            }
        }

        let app_progress = app.clone();
        let trace_progress = trace_id.clone();
        let region_name = region.name.clone();
        let filename = region.pbf_filename.clone();
        download::download_pbf(&client, region, &folder, move |done, total| {
            emit_progress(
                &app_progress,
                &trace_progress,
                "pbf",
                Some(&region_name),
                Some(&filename),
                done,
                total,
            );
        })
        .await
        .map_err(|error| fail(&mode_dir, &trace_id, STATUS_MISSING_PBF, error))?;
    }

    let pbf_paths: Vec<PathBuf> = required
        .iter()
        .map(|region| regions::pbf_path(&folder, &region.pbf_filename))
        .collect();
    let pbf_refs: Vec<PbfFileRef> = pbf_paths.iter().map(|path| pbf_ref(path)).collect();

    detect_core(
        &app,
        &mode_dir,
        &trace_id,
        &expected_hash,
        &params,
        pbf_paths,
        pbf_refs,
        &points,
        bbox,
    )
    .await
    .map_err(|error| fail(&mode_dir, &trace_id, STATUS_ERROR, error))
}

/// Détecte les tunnels sans téléchargement. Utilisé par la relance quand les
/// ressources sont déjà présentes.
#[tauri::command]
pub async fn tunnels_detect(
    app: tauri::AppHandle,
    trace_id: String,
) -> Result<TunnelPrepareResult, String> {
    let _guard = PREPARE_LOCK
        .try_lock()
        .map_err(|_| "Une préparation de tunnels est déjà en cours.".to_string())?;

    let mode_dir = get_mode_dir(&app)?;
    let (_, gpx_path, expected_hash) = resolve_trace(&mode_dir, &trace_id)?;
    let (params, support) = read_config(&app).await;
    let Some(folder) = support.pbf_directory.as_deref().map(PathBuf::from) else {
        return Err(fail(
            &mode_dir,
            &trace_id,
            STATUS_NOT_CONFIGURED,
            "Aucun dossier OSM n'est configuré.".to_string(),
        ));
    };
    if !support.actif {
        return Err(fail(
            &mode_dir,
            &trace_id,
            STATUS_NOT_CONFIGURED,
            "La détection des tunnels est désactivée.".to_string(),
        ));
    }
    if !folder.is_dir() || std::fs::read_dir(&folder).is_err() {
        return Err(fail(
            &mode_dir,
            &trace_id,
            STATUS_NOT_CONFIGURED,
            format!("Le dossier OSM est inaccessible : {}.", folder.display()),
        ));
    }

    let (points, bbox) =
        read_points_and_bbox(&gpx_path, params.bbox_margin_m)
            .map_err(|error| fail(&mode_dir, &trace_id, STATUS_ERROR, error))?;
    let Some(regions_file) = regions::load_regions(&folder) else {
        return Err(fail(
            &mode_dir,
            &trace_id,
            STATUS_NOT_CONFIGURED,
            "La liste des régions OSM (regions.json) est absente.".to_string(),
        ));
    };
    let required = regions::select_regions(&regions_file, &points);
    if required.is_empty() {
        return Err(fail(
            &mode_dir,
            &trace_id,
            STATUS_ERROR,
            "Aucune région Geofabrik ne couvre la zone de cette trace.".to_string(),
        ));
    }
    let missing: Vec<&str> = required
        .iter()
        .filter(|region| !regions::pbf_present(&folder, &region.pbf_filename))
        .map(|region| region.pbf_filename.as_str())
        .collect();
    if !missing.is_empty() {
        return Err(fail(
            &mode_dir,
            &trace_id,
            STATUS_MISSING_PBF,
            format!("Fichier(s) PBF manquant(s) : {}.", missing.join(", ")),
        ));
    }

    let pbf_paths: Vec<PathBuf> = required
        .iter()
        .map(|region| regions::pbf_path(&folder, &region.pbf_filename))
        .collect();
    let pbf_refs: Vec<PbfFileRef> = pbf_paths.iter().map(|path| pbf_ref(path)).collect();

    detect_core(
        &app,
        &mode_dir,
        &trace_id,
        &expected_hash,
        &params,
        pbf_paths,
        pbf_refs,
        &points,
        bbox,
    )
    .await
    .map_err(|error| fail(&mode_dir, &trace_id, STATUS_ERROR, error))
}

/// Sauvegarde les tunnels édités et vaut validation opérateur.
#[tauri::command]
pub async fn tunnels_save(
    app: tauri::AppHandle,
    trace_id: String,
    tunnels: Vec<Tunnel>,
) -> Result<TunnelFile, String> {
    let mode_dir = get_mode_dir(&app)?;
    let (_, gpx_path, _) = resolve_trace(&mode_dir, &trace_id)?;
    let (params, _) = read_config(&app).await;

    let points = pipeline::load_points(&gpx_path)?;
    let total_km = geometry::cumulative_distances(&points)
        .last()
        .copied()
        .unwrap_or(0.0)
        / 1000.0;
    let tunnels = normalize_tunnels(tunnels, total_km)?;

    let existing = load_tunnel_file(&mode_dir, &trace_id);
    let detected_at = existing
        .as_ref()
        .and_then(|file| file.detected_at.clone());
    let pbf_files = existing
        .as_ref()
        .map(|file| file.pbf_files.clone())
        .unwrap_or_default();
    let params = existing.map(|file| file.params).unwrap_or(params);

    let file = TunnelFile {
        version: FILE_VERSION,
        trace_id: trace_id.clone(),
        detected_at,
        updated_at: iso_now(),
        pbf_files,
        params,
        tunnels,
    };

    file::save_file(&file::file_path(&mode_dir, &trace_id), &file)?;
    set_tunnel_state(
        &mode_dir,
        &trace_id,
        STATUS_SUCCESS,
        Some(group_count(&file.tunnels)),
    );
    Ok(file)
}

/// État global du support OSM, lu par la carte du circuit.
#[tauri::command]
pub async fn tunnels_support_status(
    app: tauri::AppHandle,
) -> Result<TunnelSupportStatus, String> {
    let (_, support) = read_config(&app).await;
    let folder = support.pbf_directory.as_deref().map(PathBuf::from);
    let configured = support.actif && folder.is_some();
    let readable = folder
        .as_ref()
        .map(|path| path.is_dir() && std::fs::read_dir(path).is_ok())
        .unwrap_or(false);
    let regions_file = if readable {
        folder.as_deref().and_then(regions::load_regions)
    } else {
        None
    };
    let pbf_count = if readable {
        folder
            .as_deref()
            .map(regions::count_pbfs)
            .unwrap_or(0)
    } else {
        0
    };

    Ok(TunnelSupportStatus {
        active: support.actif,
        configured,
        folder: folder.map(|path| path.to_string_lossy().to_string()),
        readable,
        regions_present: regions_file.is_some(),
        regions_version: regions_file.map(|file| file.version),
        pbf_count,
    })
}

/// Sélecteur natif de dossier, pour le paramètre `Osm.Fichiers.dossierPbf`.
#[tauri::command]
pub async fn pick_directory(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let folder = app
        .dialog()
        .file()
        .set_title("Choisir le dossier des données OSM")
        .blocking_pick_folder();

    match folder {
        Some(path) => {
            let path = path.into_path().map_err(|error| error.to_string())?;
            Ok(Some(path.to_string_lossy().to_string()))
        }
        None => Ok(None),
    }
}
