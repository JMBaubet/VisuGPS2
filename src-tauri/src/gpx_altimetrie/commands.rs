//! Commandes Tauri du module Altimétrie.
//!
//! Quatre commandes publiques, appelées par le store Pinia `useAltimetrieStore` :
//! chargement (`altimetrie_load`), détection (`altimetrie_detect`), aperçu
//! d'édition (`altimetrie_preview`) et sauvegarde (`altimetrie_save`).
//!
//! Le fichier `traces/{trace_id}/altimetrie.json` est la seule persistance :
//! - `detect` le réécrit intégralement à partir du GPX et des paramètres ;
//! - `preview` reconstruit profil, segments et métriques avec des montées
//!   éditées, **sans écrire** ;
//! - `save` fait la même reconstruction puis écrit atomiquement.
//!
//! Le module expose aussi `generate_status`, le point d'entrée des chaînes
//! automatiques : import d'une trace déjà propre et validation d'audit. Comme
//! pour Multiride, la génération est best-effort et ne bloque jamais l'opération
//! déclenchante.

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::gpx_audit::export::iso_now;
use crate::import_gpx::{
    get_mode_dir, get_trace_gpx_path, get_traces_path, load_registry, update_registry,
};

use super::file;
use super::params::read_altimetrie_params;
use super::pipeline::{self, RawPoint};
use super::types::{AltimetrieClimbEdit, AltimetrieFile, AltimetrieParams};

// ─── Résolution de la trace et lecture du GPX ────────────────────────

/// Retrouve le nom du GPX et son chemin depuis le registre.
fn resolve_trace(mode_dir: &Path, trace_id: &str) -> Result<(String, PathBuf), String> {
    let traces_path = get_traces_path(mode_dir);
    let registry = load_registry(&traces_path);
    let trace = registry
        .iter()
        .find(|trace| trace.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable : {}.", trace_id))?;
    let gpx_path = get_trace_gpx_path(mode_dir, trace_id, &trace.filename);
    Ok((trace.filename.clone(), gpx_path))
}

/// Lit les points GPX porteurs d'une altitude, dans l'ordre du fichier.
///
/// Les points sans `<ele>` sont ignorés, comme dans le prototype. Le pipeline
/// refuse ensuite une trace de moins de deux points exploitables.
pub fn read_raw_points(gpx_path: &Path) -> Result<Vec<RawPoint>, String> {
    if !gpx_path.exists() {
        return Err(format!(
            "Fichier GPX introuvable : {}.",
            gpx_path.display()
        ));
    }
    let file = std::fs::File::open(gpx_path)
        .map_err(|e| format!("Ouverture du GPX ({}): {}", gpx_path.display(), e))?;
    let gpx = gpx::read(std::io::BufReader::new(file))
        .map_err(|e| format!("GPX illisible ({}): {}", gpx_path.display(), e))?;

    let mut points = Vec::new();
    for track in &gpx.tracks {
        for segment in &track.segments {
            for waypoint in &segment.points {
                let Some(ele) = waypoint.elevation else {
                    continue;
                };
                if !ele.is_finite() {
                    continue;
                }
                let point = waypoint.point();
                if !point.y().is_finite() || !point.x().is_finite() {
                    continue;
                }
                points.push(RawPoint {
                    lat: point.y(),
                    lon: point.x(),
                    ele,
                });
            }
        }
    }

    if points.len() < 2 {
        return Err(
            "Profil altimétrique indisponible : moins de 2 points avec altitude.".to_string(),
        );
    }
    Ok(points)
}

// ─── Validation de l'analyse par l'opérateur ─────────────────────────

/// Pose le drapeau de validation de l'altimétrie dans le registre des traces.
///
/// La mutation passe par `update_registry` : chargement, modification et
/// écriture forment une section critique, sans quoi une écriture concurrente
/// (détection des communes en tâche de fond) pourrait effacer le drapeau.
fn set_validated(mode_dir: &Path, trace_id: &str, validated: bool) -> Result<(), String> {
    update_registry(&get_traces_path(mode_dir), |registry| {
        if let Some(entry) = registry.iter_mut().find(|trace| trace.id == trace_id) {
            entry.altimetrie_validated = validated;
        }
        Ok(())
    })
}

/// Repose le drapeau de validation à `false`, en **best-effort**.
///
/// Utilisé après une régénération : l'analyse produite n'a pas été relue par
/// l'opérateur. Tolérant à une trace absente du registre (import d'une trace
/// neuve, enregistrée après la génération) et à un registre absent ; une erreur
/// d'écriture est loggée sans faire échouer la génération.
fn clear_validated(mode_dir: &Path, trace_id: &str) {
    let traces_path = get_traces_path(mode_dir);
    if !traces_path.exists() {
        return;
    }
    if let Err(error) = set_validated(mode_dir, trace_id, false) {
        eprintln!(
            "[altimetrie] invalidation de la validation impossible pour {} : {}",
            trace_id, error
        );
    }
}

// ─── Implémentations testables ────────────────────────────────────────

/// Détecte l'altimétrie, écrit le fichier et retourne son contenu.
///
/// Une détection — automatique ou « Recalculer » — produit une analyse que
/// l'opérateur n'a pas encore validée : le drapeau du registre est reposé à
/// `false`. Une trace absente du registre (import en cours) est ignorée.
pub fn detect_impl(
    mode_dir: &Path,
    trace_id: &str,
    gpx_path: &Path,
    source: &str,
    params: &AltimetrieParams,
    updated_at: &str,
) -> Result<AltimetrieFile, String> {
    let points = read_raw_points(gpx_path)?;
    let file = pipeline::detect(&points, params, trace_id, source, updated_at)?;
    file::save_file(&file::file_path(mode_dir, trace_id), &file)?;
    clear_validated(mode_dir, trace_id);
    Ok(file)
}

/// Reconstruit le fichier avec des montées éditées, sans ou avec écriture.
pub fn rebuild_impl(
    mode_dir: &Path,
    trace_id: &str,
    gpx_path: &Path,
    source: &str,
    params: &AltimetrieParams,
    updated_at: &str,
    climbs: &[AltimetrieClimbEdit],
    persist: bool,
) -> Result<AltimetrieFile, String> {
    let points = read_raw_points(gpx_path)?;
    let file = pipeline::build_file(&points, params, trace_id, source, updated_at, climbs)?;
    if persist {
        file::save_file(&file::file_path(mode_dir, trace_id), &file)?;
    }
    Ok(file)
}

/// Reconstruit l'analyse avec les montées éditées, l'écrit et **valide** le
/// résultat dans le registre.
///
/// C'est la seule opération qui pose `altimetrie_validated = true` : la vue
/// n'appelle `save` que sur un geste explicite de l'opérateur — bouton
/// « Valider » ou « Sauvegarder ». Une détection, elle, ne vaut pas validation.
pub fn save_impl(
    mode_dir: &Path,
    trace_id: &str,
    gpx_path: &Path,
    source: &str,
    params: &AltimetrieParams,
    updated_at: &str,
    climbs: &[AltimetrieClimbEdit],
) -> Result<AltimetrieFile, String> {
    let file = rebuild_impl(
        mode_dir,
        trace_id,
        gpx_path,
        source,
        params,
        updated_at,
        climbs,
        true,
    )?;
    set_validated(mode_dir, trace_id, true)?;
    Ok(file)
}

/// Génération d'une **chaîne automatique** — import d'une trace propre,
/// validation d'audit.
///
/// Best-effort assumé : un échec (altitude absente, trace dégénérée, panic) est
/// loggé et ne remonte jamais. Un éventuel fichier périmé est supprimé avant la
/// tentative : si le GPX vient d'être réécrit, l'ancienne analyse ne décrit plus
/// la trace courante. La vue relancera la détection si le fichier manque.
pub fn generate_status(
    mode_dir: &Path,
    trace_id: &str,
    gpx_path: &Path,
    source: &str,
    params: AltimetrieParams,
) -> bool {
    file::remove_file(mode_dir, trace_id);

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        detect_impl(mode_dir, trace_id, gpx_path, source, &params, &iso_now())
    }));

    match outcome {
        Ok(Ok(file)) => {
            println!(
                "[altimetrie] génération trace={} points={} montées={} segments={}",
                trace_id,
                file.trace.total_points,
                file.climbs.len(),
                file.segments.len()
            );
            true
        }
        Ok(Err(error)) => {
            eprintln!(
                "[altimetrie] génération non jouée pour {} : {}",
                trace_id, error
            );
            false
        }
        Err(_) => {
            eprintln!(
                "[altimetrie] génération interrompue pour {} : panique",
                trace_id
            );
            false
        }
    }
}

// ─── Commandes Tauri ──────────────────────────────────────────────────

/// Relit le fichier d'analyse d'une trace.
///
/// `None` si le fichier est absent, illisible, d'une version inconnue ou
/// rattaché à une autre trace : la vue relance alors la détection.
#[tauri::command]
pub async fn altimetrie_load(
    app: tauri::AppHandle,
    trace_id: String,
) -> Result<Option<AltimetrieFile>, String> {
    let mode_dir = get_mode_dir(&app)?;
    Ok(file::load_file(
        &file::file_path(&mode_dir, &trace_id),
        &trace_id,
    ))
}

/// Détecte l'altimétrie avec les paramètres fournis et écrit le fichier.
///
/// Utilisée à l'ouverture de la vue quand le fichier manque, et par le bouton
/// « Recalculer » après modification des réglages.
#[tauri::command]
pub async fn altimetrie_detect(
    app: tauri::AppHandle,
    trace_id: String,
    params: AltimetrieParams,
) -> Result<AltimetrieFile, String> {
    let mode_dir = get_mode_dir(&app)?;
    let (source, gpx_path) = resolve_trace(&mode_dir, &trace_id)?;
    let started = Instant::now();
    let file = detect_impl(
        &mode_dir,
        &trace_id,
        &gpx_path,
        &source,
        &params,
        &iso_now(),
    )?;

    println!(
        "[altimetrie] detect trace={} points={} montées={} segments={} durée={}ms",
        trace_id,
        file.trace.total_points,
        file.climbs.len(),
        file.segments.len(),
        started.elapsed().as_millis()
    );
    Ok(file)
}

/// Reconstruit l'analyse avec des montées éditées, **sans écrire**.
///
/// Les paramètres utilisés sont ceux du fichier existant : modifier les réglages
/// ne suffit pas à changer l'analyse courante, il faut passer par « Recalculer ».
#[tauri::command]
pub async fn altimetrie_preview(
    app: tauri::AppHandle,
    trace_id: String,
    climbs: Vec<AltimetrieClimbEdit>,
) -> Result<AltimetrieFile, String> {
    let mode_dir = get_mode_dir(&app)?;
    let existing = file::load_file(&file::file_path(&mode_dir, &trace_id), &trace_id)
        .ok_or_else(|| {
            format!(
                "Altimétrie absente pour la trace {} : relancez la détection.",
                trace_id
            )
        })?;
    let (source, gpx_path) = resolve_trace(&mode_dir, &trace_id)?;

    let file = rebuild_impl(
        &mode_dir,
        &trace_id,
        &gpx_path,
        &source,
        &existing.params,
        &existing.updated_at,
        &climbs,
        false,
    )?;

    println!(
        "[altimetrie] preview trace={} montées={} (non persistée)",
        trace_id,
        file.climbs.len()
    );
    Ok(file)
}

/// Reconstruit l'analyse avec des montées éditées et l'écrit.
#[tauri::command]
pub async fn altimetrie_save(
    app: tauri::AppHandle,
    trace_id: String,
    climbs: Vec<AltimetrieClimbEdit>,
) -> Result<AltimetrieFile, String> {
    let mode_dir = get_mode_dir(&app)?;
    let existing = file::load_file(&file::file_path(&mode_dir, &trace_id), &trace_id)
        .ok_or_else(|| {
            format!(
                "Altimétrie absente pour la trace {} : relancez la détection.",
                trace_id
            )
        })?;
    let (source, gpx_path) = resolve_trace(&mode_dir, &trace_id)?;
    let started = Instant::now();

    let file = save_impl(
        &mode_dir,
        &trace_id,
        &gpx_path,
        &source,
        &existing.params,
        &iso_now(),
        &climbs,
    )?;

    println!(
        "[altimetrie] save trace={} montées={} durée={}ms",
        trace_id,
        file.climbs.len(),
        started.elapsed().as_millis()
    );
    Ok(file)
}

/// Lit les paramètres `Altimetrie.*` pour les chaînes automatiques.
///
/// Réexport utilitaire : l'import et la validation d'audit n'ont pas à connaître
/// le module `params`.
pub fn read_params(app: &tauri::AppHandle) -> AltimetrieParams {
    read_altimetrie_params(app)
}
