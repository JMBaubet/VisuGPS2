//! Commandes Tauri de gestion des données OSM partagées.
//!
//! Elles alimentent l'icône d'état de l'Accueil et la fenêtre de gestion :
//! inventaire, catalogue Geofabrik, taille distante, téléchargement,
//! renouvellement, suppression et rafraîchissement du catalogue.
//!
//! Les opérations manuelles sont sérialisées par `OSM_LOCK` ; les
//! téléchargements eux-mêmes sont sérialisés par le verrou de
//! `gpx_tunnels::download`, partagé avec la préparation silencieuse des
//! tunnels.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use tauri::Emitter;

use super::inventory;
use super::params::{read_settings, OsmSettings};
use super::types::{
    OsmDownloadResult, OsmPresentRegionGeometry, OsmProgress, OsmRegionCatalog,
    OsmRegionGeometry, OsmRegionOption, OsmRegionProbe, OsmSupportOverview,
};
use crate::gpx_tunnels::download;
use crate::gpx_tunnels::regions;
use crate::gpx_tunnels::types::{RegionEntry, RegionsFile};

const PROGRESS_EVENT: &str = "osm-progress";
const STAGE_CATALOG: &str = "catalog";
const STAGE_DOWNLOAD: &str = "download";

/// Une seule opération manuelle OSM à la fois (téléchargement, suppression,
/// rafraîchissement du catalogue).
static OSM_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// État global du support OSM, lu par l'icône de la toolbar.
#[tauri::command]
pub async fn osm_support_overview(
    app: tauri::AppHandle,
) -> Result<OsmSupportOverview, String> {
    let settings = read_settings(&app).await;
    Ok(inventory::build_overview(&settings, Utc::now()))
}

/// Catalogue Geofabrik local (`regions.json`), avec présence de chaque PBF.
#[tauri::command]
pub async fn osm_region_catalog(
    app: tauri::AppHandle,
) -> Result<OsmRegionCatalog, String> {
    let settings = read_settings(&app).await;
    let Some(folder) = settings.pbf_directory.as_deref() else {
        return Ok(empty_catalog());
    };
    if !folder.is_dir() || std::fs::read_dir(folder).is_err() {
        return Ok(empty_catalog());
    }
    let Some(file) = regions::load_regions(folder) else {
        return Ok(empty_catalog());
    };
    Ok(catalog_from(folder, &file))
}

/// Taille distante d'une région (HEAD `Content-Length`) et espace disponible.
#[tauri::command]
pub async fn osm_region_size(
    app: tauri::AppHandle,
    region_id: String,
) -> Result<OsmRegionProbe, String> {
    let settings = read_settings(&app).await;
    let folder = folder_from(&settings)?;
    let catalog = read_catalog(&folder)?;
    let region = find_region(&catalog, &region_id)?;
    let client = download::client()?;
    let bytes = download::remote_size(&client, &region.pbf_url).await;
    let available_bytes = fs4::available_space(&folder).ok();
    Ok(OsmRegionProbe {
        bytes,
        available_bytes,
    })
}

/// Géométrie GeoJSON simplifiée d'une région pour l'aperçu cartographique.
///
/// Retourne le polygone de la bbox si `regions-geometry.json` n'est pas encore
/// présent (catalogue généré avant l'ajout des géométries).
#[tauri::command]
pub async fn osm_region_geometry(
    app: tauri::AppHandle,
    region_id: String,
) -> Result<Option<OsmRegionGeometry>, String> {
    let settings = read_settings(&app).await;
    let Some(folder) = settings.pbf_directory.as_deref() else {
        return Ok(None);
    };
    let Some(catalog) = regions::load_regions(folder) else {
        return Ok(None);
    };
    let Some(region) = catalog
        .regions
        .iter()
        .find(|region| region.id == region_id)
    else {
        return Ok(None);
    };

    let from_file = regions::load_region_geometries(folder).and_then(|file| {
        file.regions
            .into_iter()
            .find(|entry| entry.id == region_id)
            .map(|entry| entry.geometry)
    });
    let approximate = from_file.is_none();
    let geometry = from_file.unwrap_or_else(|| regions::bbox_geometry(region.bbox));

    Ok(Some(OsmRegionGeometry {
        region_id,
        bbox: region.bbox,
        geometry,
        approximate,
    }))
}

/// Géométries de toutes les régions dont le PBF est présent, avec leur état.
///
/// Utilisé à l'ouverture de la fenêtre « Données OSM » pour colorer les
/// régions présentes en vert/jaune/orange sur la carte.
#[tauri::command]
pub async fn osm_present_region_geometries(
    app: tauri::AppHandle,
) -> Result<Vec<OsmPresentRegionGeometry>, String> {
    let settings = read_settings(&app).await;
    Ok(present_region_geometries(&settings, Utc::now()))
}

/// Construit les géométries des régions présentes (fonction pure hors disque).
pub fn present_region_geometries(
    settings: &OsmSettings,
    now: chrono::DateTime<Utc>,
) -> Vec<OsmPresentRegionGeometry> {
    let Some(folder) = settings.pbf_directory.as_deref() else {
        return Vec::new();
    };
    let overview = inventory::build_overview(settings, now);
    let Some(catalog) = regions::load_regions(folder) else {
        return Vec::new();
    };

    let mut geometry_by_id: HashMap<String, serde_json::Value> = HashMap::new();
    if let Some(geometries) = regions::load_region_geometries(folder) {
        for entry in geometries.regions {
            geometry_by_id.insert(entry.id, entry.geometry);
        }
    }

    let mut out = Vec::new();
    for file in overview.files {
        let Some(region_id) = file.region_id else {
            continue;
        };
        let Some(region) = catalog
            .regions
            .iter()
            .find(|region| region.id == region_id)
        else {
            continue;
        };
        let from_file = geometry_by_id.get(&region_id).cloned();
        let approximate = from_file.is_none();
        let geometry = from_file.unwrap_or_else(|| regions::bbox_geometry(region.bbox));
        out.push(OsmPresentRegionGeometry {
            region_id,
            state: file.state,
            bbox: region.bbox,
            geometry,
            approximate,
        });
    }
    out
}

/// Télécharge ou renouvelle un PBF après contrôle du plafond et de l'espace.
#[tauri::command]
pub async fn osm_download(
    app: tauri::AppHandle,
    region_id: String,
    renew: Option<bool>,
    ignore_size_limit: Option<bool>,
) -> Result<OsmDownloadResult, String> {
    let _guard = OSM_LOCK.try_lock().map_err(|_| operation_busy())?;
    let settings = read_settings(&app).await;
    let folder = folder_from(&settings)?;
    let catalog = read_catalog(&folder)?;
    let region = find_region(&catalog, &region_id)?.clone();
    let renew = renew.unwrap_or(false);
    let ignore_size_limit = ignore_size_limit.unwrap_or(false);
    let target = regions::pbf_path(&folder, &region.pbf_filename);

    if regions::pbf_present(&folder, &region.pbf_filename) && !renew {
        let size_bytes = std::fs::metadata(&target)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        return Ok(OsmDownloadResult {
            filename: region.pbf_filename,
            size_bytes,
        });
    }

    let client = download::client()?;
    let bytes = download::remote_size(&client, &region.pbf_url)
        .await
        .ok_or_else(|| {
            format!(
                "Taille distante inconnue pour {}. Vérifiez la connexion puis réessayez.",
                region.name
            )
        })?;

    let available = fs4::available_space(&folder).map_err(|error| {
        format!(
            "Espace disponible illisible sur le support OSM : {}.",
            error
        )
    })?;
    check_download_limits(
        &region.name,
        bytes,
        available,
        settings.max_pbf_mb,
        ignore_size_limit,
    )?;

    let app_progress = app.clone();
    let progress_region_id = region.id.clone();
    let progress_region_name = region.name.clone();
    let progress_filename = region.pbf_filename.clone();
    download::download_to_file(&client, &region.pbf_url, &target, move |done, total| {
        emit_progress(
            &app_progress,
            STAGE_DOWNLOAD,
            Some(&progress_region_id),
            Some(&progress_region_name),
            Some(&progress_filename),
            done,
            total,
        );
    })
    .await?;

    Ok(OsmDownloadResult {
        filename: region.pbf_filename,
        size_bytes: bytes,
    })
}

/// Supprime un PBF du support, après validation stricte du nom.
#[tauri::command]
pub async fn osm_delete(app: tauri::AppHandle, filename: String) -> Result<(), String> {
    let _guard = OSM_LOCK.try_lock().map_err(|_| operation_busy())?;
    validate_filename(&filename)?;
    let settings = read_settings(&app).await;
    let folder = folder_from(&settings)?;
    let target = regions::pbf_path(&folder, &filename);
    if !target.is_file() {
        return Err(format!("Fichier introuvable : {}.", filename));
    }
    tokio::fs::remove_file(&target)
        .await
        .map_err(|error| format!("Suppression de {} impossible : {}", filename, error))?;
    let _ = tokio::fs::remove_file(download::part_path(&target)).await;
    Ok(())
}

/// Retélécharge l'index Geofabrik et réécrit `regions.json` atomiquement.
#[tauri::command]
pub async fn osm_catalog_refresh(
    app: tauri::AppHandle,
) -> Result<OsmRegionCatalog, String> {
    let _guard = OSM_LOCK.try_lock().map_err(|_| operation_busy())?;
    let settings = read_settings(&app).await;
    let folder = folder_from(&settings)?;
    let client = download::client()?;
    let app_progress = app.clone();
    let file = download::ensure_regions_json(
        &client,
        &folder,
        regions::INDEX_URL,
        true,
        move |done, total| {
            emit_progress(
                &app_progress,
                STAGE_CATALOG,
                None,
                None,
                Some(regions::REGIONS_FILE_NAME),
                done,
                total,
            );
        },
    )
    .await?;
    Ok(catalog_from(&folder, &file))
}

/// Vérifie le plafond de taille et l'espace disponible avant un téléchargement
/// manuel. Le plafond peut être franchi avec `ignore_size_limit` ; l'espace
/// insuffisant est toujours bloquant.
pub fn check_download_limits(
    region_name: &str,
    bytes: u64,
    available: u64,
    max_pbf_mb: u64,
    ignore_size_limit: bool,
) -> Result<(), String> {
    if !ignore_size_limit && max_pbf_mb > 0 {
        let max_bytes = max_pbf_mb.saturating_mul(1_048_576);
        if bytes > max_bytes {
            return Err(format!(
                "La région {} ({}) dépasse le plafond de {} Mo. Utilisez « Télécharger quand même » ou augmentez Osm.Fichiers.tailleMaxPbf.",
                region_name,
                format_bytes(bytes),
                max_pbf_mb
            ));
        }
    }
    if bytes > available {
        return Err(format!(
            "Espace insuffisant sur le support OSM : {} requis, {} disponibles.",
            format_bytes(bytes),
            format_bytes(available)
        ));
    }
    Ok(())
}

/// Valide un nom de fichier PBF destiné à une suppression. Refuse les chemins,
/// les traversées (`..`), les fichiers non PBF et le catalogue.
pub fn validate_filename(filename: &str) -> Result<(), String> {
    let invalid = || "Nom de fichier PBF invalide.".to_string();
    if filename.is_empty()
        || filename == "."
        || filename == ".."
        || filename == regions::REGIONS_FILE_NAME
        || filename.contains('/')
        || filename.contains('\\')
    {
        return Err(invalid());
    }
    let path = Path::new(filename);
    if path.file_name().and_then(|name| name.to_str()) != Some(filename)
        || path.extension().and_then(|extension| extension.to_str()) != Some("pbf")
    {
        return Err(invalid());
    }
    Ok(())
}

/// Construit le catalogue IPC à partir de `regions.json` et du dossier local.
pub fn catalog_from(folder: &Path, file: &RegionsFile) -> OsmRegionCatalog {
    let regions = file
        .regions
        .iter()
        .map(|region| {
            let path = regions::pbf_path(folder, &region.pbf_filename);
            let metadata = std::fs::metadata(&path)
                .ok()
                .filter(|metadata| metadata.is_file());
            let local_created_at = metadata
                .as_ref()
                .and_then(|metadata| inventory::file_date(&path, metadata).0)
                .map(|date| date.to_rfc3339());
            OsmRegionOption {
                id: region.id.clone(),
                name: region.name.clone(),
                country_code: region.country_code.clone(),
                country_name: region.country_name.clone(),
                continent: region.continent.clone(),
                continent_name: region.continent_name.clone(),
                pbf_filename: region.pbf_filename.clone(),
                size_mb: region.size_mb,
                present: metadata
                    .as_ref()
                    .map(|metadata| metadata.len() > 0)
                    .unwrap_or(false),
                local_size_bytes: metadata.as_ref().map(|metadata| metadata.len()),
                local_created_at,
            }
        })
        .collect();

    OsmRegionCatalog {
        present: true,
        version: Some(file.version.clone()),
        regions,
    }
}

fn empty_catalog() -> OsmRegionCatalog {
    OsmRegionCatalog {
        present: false,
        version: None,
        regions: Vec::new(),
    }
}

fn operation_busy() -> String {
    "Une opération OSM est déjà en cours.".to_string()
}

fn folder_from(settings: &OsmSettings) -> Result<PathBuf, String> {
    let Some(folder) = settings.pbf_directory.clone() else {
        return Err("Aucun dossier OSM n'est configuré.".to_string());
    };
    if !folder.is_dir() || std::fs::read_dir(&folder).is_err() {
        return Err(format!(
            "Support OSM introuvable : {}. Vérifiez que la clé USB est branchée.",
            folder.display()
        ));
    }
    Ok(folder)
}

fn read_catalog(folder: &Path) -> Result<RegionsFile, String> {
    regions::load_regions(folder).ok_or_else(|| {
        format!(
            "Le catalogue regions.json est absent ou illisible ({}). Utilisez « Mettre à jour le catalogue ».",
            regions::regions_path(folder).display()
        )
    })
}

fn find_region<'a>(
    catalog: &'a RegionsFile,
    region_id: &str,
) -> Result<&'a RegionEntry, String> {
    catalog
        .regions
        .iter()
        .find(|region| region.id == region_id)
        .ok_or_else(|| format!("Région inconnue : {}.", region_id))
}

fn emit_progress(
    app: &tauri::AppHandle,
    stage: &str,
    region_id: Option<&str>,
    region_name: Option<&str>,
    filename: Option<&str>,
    bytes_done: u64,
    bytes_total: u64,
) {
    let percent = if bytes_total > 0 {
        (bytes_done as f64 * 100.0 / bytes_total as f64).min(100.0)
    } else {
        0.0
    };
    let progress = OsmProgress {
        stage: stage.to_string(),
        region_id: region_id.map(str::to_string),
        region_name: region_name.map(str::to_string),
        filename: filename.map(str::to_string),
        bytes_done,
        bytes_total,
        percent,
    };
    if let Err(error) = app.emit(PROGRESS_EVENT, progress) {
        eprintln!("[osm] progression non émise : {}", error);
    }
}

fn format_bytes(bytes: u64) -> String {
    let mb = bytes as f64 / 1_048_576.0;
    if mb >= 1024.0 {
        format!("{:.2} Go", mb / 1024.0)
    } else {
        format!("{:.1} Mo", mb)
    }
}
