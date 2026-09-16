//! Commandes Tauri du module Audit GPX (Livrable 4).
//!
//! Sept commandes publiques, appelées par le store Pinia `useAuditStore`
//! (Livrable 2). Aucune n'entretient d'état côté Rust : la trace de travail et
//! les findings vivent dans le front, chaque appel est une fonction pure qui
//! reçoit ce dont elle a besoin et retourne le nouvel état.
//!
//! Chaque commande est un **adaptateur mince** : la logique réside dans une
//! fonction interne (`detection_impl`, `validate_impl`, …) qui ne dépend pas de
//! Tauri et reste donc testable directement (Livrable 4 §6).

use std::path::Path;
use std::sync::{Arc, RwLock};

use tauri::Manager;

use crate::import_gpx::{
    build_geojson_feature, compute_file_hash, compute_stats, extract_line_coordinates,
    get_geojson_path, get_mode_dir, get_trace_gpx_path, get_traces_path, load_registry,
    save_registry, TraceMetadata,
};
use crate::settings::{get_toml_value_by_path, SettingsState};

use super::archive::{self, AuditArchive};
use super::export::{rewrite_gpx, FindingsSummary};
use super::overlay::{map_overlays, FindingOverlay};
use super::pipeline;
use super::preview::{delete_preview, DeletePreview};
use super::routing::routes_identical;
use super::types::{
    AuditDetectionResult, AuditParams, AuditPoint, AuditState, CorrectionType, Finding,
    FindingStatus, LatLon,
};

// ─── Lecture des paramètres ───────────────────────────────────────────

/// Lit les paramètres du détecteur (`Audit.*`) avec repli sur les valeurs par
/// défaut du livrable 6. Ne **panique jamais** (`try_state`) : l'import
/// s'appuie dessus, une défaillance du système de paramètres ne doit pas
/// bloquer l'import.
pub fn read_audit_params(app: &tauri::AppHandle) -> AuditParams {
    let state = match app.try_state::<Arc<RwLock<SettingsState>>>() {
        Some(s) => s,
        None => return default_audit_params(),
    };
    let guard = match state.read() {
        Ok(g) => g,
        Err(_) => return default_audit_params(),
    };
    audit_params_from_settings(&guard.default_toml, &guard.user_overrides)
}

/// Valeurs par défaut des paramètres d'audit (miroir de `settings.default.toml`).
pub fn default_audit_params() -> AuditParams {
    AuditParams {
        consol_m: 0.5,
        tol_deg: 20.0,
        pair_m: 50.0,
        maxpairs: 5,
        seg_m: 200.0,
        close_m: 15.0,
        angle_deg: 270,
    }
}

/// Résout les paramètres depuis les tables TOML (surcharge utilisateur
/// prioritaire, puis valeur par défaut, puis repli). Pure et testable.
pub fn audit_params_from_settings(
    default_toml: &toml::Table,
    user_overrides: &toml::Table,
) -> AuditParams {
    let fallback = default_audit_params();
    let f = |path: &str, def: f64| -> f64 {
        let value = get_toml_value_by_path(user_overrides, path)
            .or_else(|| get_toml_value_by_path(default_toml, path));
        match value {
            Some(toml::Value::Float(v)) => *v,
            Some(toml::Value::Integer(v)) => *v as f64,
            _ => def,
        }
    };
    let i = |path: &str, def: u32| -> u32 {
        let value = get_toml_value_by_path(user_overrides, path)
            .or_else(|| get_toml_value_by_path(default_toml, path));
        match value {
            Some(toml::Value::Integer(v)) if *v >= 0 => *v as u32,
            Some(toml::Value::Float(v)) if *v >= 0.0 => *v as u32,
            _ => def,
        }
    };

    AuditParams {
        consol_m: f("Audit.Consolidation.seuil", fallback.consol_m),
        tol_deg: f("Audit.AR.toleranceDeg", fallback.tol_deg),
        pair_m: f("Audit.AR.seuilPaireM", fallback.pair_m),
        maxpairs: i("Audit.AR.maxPaires", fallback.maxpairs),
        seg_m: f("Audit.AR.branchesMaxM", fallback.seg_m),
        close_m: f("Audit.RP.seuilFermetureM", fallback.close_m),
        angle_deg: i("Audit.RP.angleMinDeg", fallback.angle_deg),
    }
}

/// Lit `Audit.Application.nom` (nom d'application de l'export GPX). Repli sur
/// chaîne vide : `export::rewrite_gpx` applique alors son propre défaut
/// (`VérificationGPX`), y compris si le paramètre n'existe pas encore.
fn read_app_name(app: &tauri::AppHandle) -> String {
    let state = match app.try_state::<Arc<RwLock<SettingsState>>>() {
        Some(s) => s,
        None => return String::new(),
    };
    let guard = match state.read() {
        Ok(g) => g,
        Err(_) => return String::new(),
    };
    let value = get_toml_value_by_path(&guard.user_overrides, "Audit.Application.nom")
        .or_else(|| get_toml_value_by_path(&guard.default_toml, "Audit.Application.nom"));
    match value {
        Some(toml::Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}

// ─── 1. Détection ─────────────────────────────────────────────────────

/// Lance la détection AR + RP sur la trace indiquée.
///
/// Charge le GPX de la trace, consolide, construit la géométrie métrique,
/// exécute `detect_ar` puis `detect_rp`, fusionne et renumérote les findings.
#[tauri::command]
pub async fn audit_run_detection(
    app: tauri::AppHandle,
    trace_id: String,
    params: AuditParams,
) -> Result<AuditDetectionResult, String> {
    let mode_dir = get_mode_dir(&app)?;
    let registry = load_registry(&get_traces_path(&mode_dir));
    let trace = registry
        .iter()
        .find(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable : {}.", trace_id))?;
    let gpx_path = get_trace_gpx_path(&mode_dir, &trace_id, &trace.filename);

    let result = detection_impl(&gpx_path, &trace_id, &params)?;

    println!(
        "[audit] detection trace={} points={} findings={} durée={}ms",
        trace_id,
        result.points.len(),
        result.findings.len(),
        result.duration_ms
    );
    Ok(result)
}

/// Implémentation testable de `audit_run_detection` (sans `AppHandle`).
pub fn detection_impl(
    gpx_path: &Path,
    trace_id: &str,
    params: &AuditParams,
) -> Result<AuditDetectionResult, String> {
    let started = std::time::Instant::now();
    let raw = pipeline::load_gpx_points(gpx_path)?;
    let outcome = pipeline::detect_all(raw, params)?;
    let duration_ms = started.elapsed().as_millis() as u64;

    Ok(AuditDetectionResult {
        trace_id: trace_id.to_string(),
        points: outcome.points,
        total_distance_m: outcome.total_distance_m,
        findings: outcome.findings,
        params: params.clone(),
        duration_ms,
    })
}

// ─── 8. Éléments de rendu ─────────────────────────────────────────────

/// Calcule les éléments de rendu des anomalies : ancres de routage (boucles RP)
/// et étiquettes des points.
///
/// Le calcul est **lazy**, à l'image de `rpAnchors` / `showStandardLabels` du
/// HTML de référence : il est refait à chaque rendu de carte, sur la trace de
/// travail courante — les indices des findings y sont donc interprétés dans son
/// espace (invariant C4).
#[tauri::command]
pub fn audit_map_overlay(
    points: Vec<AuditPoint>,
    findings: Vec<Finding>,
    close_m: f64,
) -> Result<Vec<FindingOverlay>, String> {
    map_overlays(&points, &findings, close_m)
}

// ─── 10. Identité des tracés ORS ──────────────────────────────────────

/// Compare les deux tracés ORS (IHM §7.3).
///
/// Deux critères requis : longueurs à 2 % près, et distance de Hausdorff
/// discrète point→segment ≤ 15 m dans les deux sens. Le calcul est métrique, il
/// reste donc en Rust ; la requête réseau vit dans la composable `useAuditOrs`.
#[tauri::command]
pub fn audit_routes_identical(
    car: Vec<LatLon>,
    car_distance: f64,
    bike: Vec<LatLon>,
    bike_distance: f64,
) -> bool {
    routes_identical(&car, car_distance, &bike, bike_distance)
}

// ─── 9. Aperçu de suppression ─────────────────────────────────────────

/// Calcule l'aperçu d'une suppression de points sur `[start..=end]`.
///
/// L'aperçu est **prospectif** et sans effet : la trace de travail n'est pas
/// modifiée. Il est recalculé à chaque mouvement de curseur, à l'image de
/// `delUpdate` / `rpDelUpdate` du HTML de référence.
#[tauri::command]
pub fn audit_delete_preview(
    points: Vec<AuditPoint>,
    finding: Finding,
    start: usize,
    end: usize,
    close_m: f64,
) -> Result<DeletePreview, String> {
    delete_preview(&points, &finding, start, end, close_m)
}

// ─── 2. Suppression de points ─────────────────────────────────────────

/// Supprime la plage `[ds..=de]` et met à jour le finding associé.
///
/// `trace_id` n'est pas utilisé (la trace n'est réécrite qu'à la validation) :
/// il est accepté pour l'uniformité du contrat et les logs.
#[tauri::command]
pub fn audit_apply_delete(
    trace_id: String,
    points: Vec<AuditPoint>,
    findings: Vec<Finding>,
    finding_id: String,
    ds: usize,
    de: usize,
    next_point_id: u32,
) -> Result<AuditState, String> {
    let state = super::corrections::apply_delete(points, findings, &finding_id, ds, de, next_point_id)?;
    println!(
        "[audit] apply_delete trace={} finding={} plage=[{}..{}] -> {} points",
        trace_id,
        finding_id,
        ds,
        de,
        state.points.len()
    );
    Ok(state)
}

// ─── 3. Routage ───────────────────────────────────────────────────────

/// Remplace l'intérieur `[start+1..end-1]` par les points du tracé ORS.
///
/// `coords` contient le tracé complet renvoyé par ORS (ancres incluses) ; les
/// deux extrémités sont retirées côté `corrections::apply_route` pour éviter
/// les doublons avec `points[start]` et `points[end]`.
#[tauri::command]
pub fn audit_apply_route(
    trace_id: String,
    points: Vec<AuditPoint>,
    findings: Vec<Finding>,
    finding_id: String,
    start: usize,
    end: usize,
    coords: Vec<LatLon>,
    profile: String,
    next_point_id: u32,
) -> Result<AuditState, String> {
    let state = super::corrections::apply_route(
        points, findings, &finding_id, start, end, coords, &profile, next_point_id,
    )?;
    println!(
        "[audit] apply_route trace={} finding={} ancres=[{}..{}] profil={} -> {} points",
        trace_id,
        finding_id,
        start,
        end,
        profile,
        state.points.len()
    );
    Ok(state)
}

// ─── 4. Faux positif ──────────────────────────────────────────────────

/// Marque un finding comme faux positif (trace inchangée).
#[tauri::command]
pub fn audit_mark_fp(
    findings: Vec<Finding>,
    finding_id: String,
) -> Result<Vec<Finding>, String> {
    super::corrections::mark_fp(findings, &finding_id)
}

/// Retire le marqueur faux positif (retour à `pending`).
#[tauri::command]
pub fn audit_unmark_fp(
    findings: Vec<Finding>,
    finding_id: String,
) -> Result<Vec<Finding>, String> {
    super::corrections::unmark_fp(findings, &finding_id)
}

// ─── 5. Annulation ────────────────────────────────────────────────────

/// Annule la correction d'un finding (undo par instantané).
#[tauri::command]
pub fn audit_undo_correction(
    trace_id: String,
    points: Vec<AuditPoint>,
    findings: Vec<Finding>,
    finding_id: String,
) -> Result<AuditState, String> {
    let state = super::corrections::undo_correction(points, findings, &finding_id)?;
    println!(
        "[audit] undo trace={} finding={} -> {} points",
        trace_id,
        finding_id,
        state.points.len()
    );
    Ok(state)
}

// ─── 6. Validation ────────────────────────────────────────────────────

/// Valide l'audit : réécrit le GPX, pose `audit_status = "clean"` et enchaîne la
/// détection des passages multiples sur le GPX corrigé.
///
/// **Point de non-retour.** Refuse tant qu'un finding est `pending`.
#[tauri::command]
pub async fn audit_validate(
    app: tauri::AppHandle,
    trace_id: String,
    points: Vec<AuditPoint>,
    findings: Vec<Finding>,
) -> Result<serde_json::Value, String> {
    let mode_dir = get_mode_dir(&app)?;
    let app_name = read_app_name(&app);
    let multiride_params = crate::gpx_multiride::commands::read_multiride_params(&app);

    let updated = validate_impl(
        &mode_dir,
        &trace_id,
        &points,
        &findings,
        &app_name,
        multiride_params,
    )?;

    println!(
        "[audit] validate trace={} points={} findings={} ({} FP)",
        trace_id,
        points.len(),
        findings.len(),
        findings
            .iter()
            .filter(|f| f.status == FindingStatus::Fp)
            .count()
    );

    serde_json::to_value(&updated).map_err(|e| format!("Sérialisation de la trace : {}", e))
}

/// Implémentation testable de `audit_validate` (sans `AppHandle`).
///
/// Ordre imposé par le livrable 4 §2.7 : le GPX est réécrit **avant** la mise à
/// jour de `traces.json` (point de vigilance 6 — un échec de réécriture laisse
/// `audit_status` intact).
///
/// `multiride_params` porte les réglages `Multiride.Detection` : la détection des
/// passages multiples suit l'audit et doit porter sur le GPX corrigé, donc être
/// jouée ici — dans la commande qui le réécrit — et non après coup.
pub fn validate_impl(
    mode_dir: &Path,
    trace_id: &str,
    points: &[AuditPoint],
    findings: &[Finding],
    app_name: &str,
    multiride_params: crate::gpx_multiride::types::MultirideParams,
) -> Result<TraceMetadata, String> {
    // 1. Aucune anomalie ne doit rester à traiter.
    let pending = findings
        .iter()
        .filter(|f| f.status == FindingStatus::Pending)
        .count();
    if pending > 0 {
        return Err(format!(
            "Toutes les anomalies doivent être traitées avant de valider l'audit ({} restante(s)).",
            pending
        ));
    }

    // 2. Trace dégénérée.
    if points.len() < 2 {
        return Err("Trace dégénérée : moins de 2 points.".to_string());
    }

    // 3. Chemins et métadonnée courante.
    let traces_path = get_traces_path(mode_dir);
    let registry = load_registry(&traces_path);
    let trace = registry
        .iter()
        .find(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable : {}.", trace_id))?
        .clone();
    let gpx_path = get_trace_gpx_path(mode_dir, trace_id, &trace.filename);

    // 4. Entête source du GPX courant (réémise telle quelle).
    let ctx = pipeline::read_source_context(&gpx_path)?;

    // 5. Réécriture du GPX — le backup `.orig` est posé par `rewrite_gpx`,
    //    une seule fois, jamais écrasé.
    let summary = findings_summary(findings);
    rewrite_gpx(
        &gpx_path,
        points,
        ctx.meta_xml.as_deref(),
        ctx.attrs.as_deref(),
        ctx.trk_name.as_deref(),
        app_name,
        summary,
    )?;

    // 6-8. Régénération des dérivés depuis le GPX réécrit.
    let file = std::fs::File::open(&gpx_path)
        .map_err(|e| format!("Ouverture du GPX réécrit ({}): {}", gpx_path.display(), e))?;
    let gpx = gpx::read(std::io::BufReader::new(file))
        .map_err(|e| format!("GPX réécrit invalide ({}): {}", gpx_path.display(), e))?;
    let (stats, _) = compute_stats(&gpx)?;
    let coords = extract_line_coordinates(&gpx)?;
    let feature = build_geojson_feature(coords, trace_id, &trace.name);
    let geojson_content = serde_json::to_string_pretty(&feature)
        .map_err(|e| format!("Sérialisation GeoJSON : {}", e))?;
    pipeline::write_atomic(&get_geojson_path(mode_dir, trace_id), geojson_content.as_bytes())?;
    let hash = compute_file_hash(&gpx_path)?;

    // 8bis. Détection des passages multiples du GPX **corrigé**.
    //
    //        Elle suit l'audit dans le parcours d'une trace : la jouer ici, dans
    //        la commande qui vient de réécrire le GPX, garantit qu'elle porte sur
    //        le bon fichier et qu'aucun état intermédiaire n'oppose un GPX
    //        corrigé à un statut périmé. Best-effort : une défaillance laisse le
    //        statut vide, donc permissif, et ne remet pas en cause l'audit.
    let multiride_status = crate::gpx_multiride::commands::detect_status(
        mode_dir,
        trace_id,
        &trace.filename,
        &gpx_path,
        multiride_params,
    );

    // 9. Registre : statut d'audit, stats et hash.
    let mut registry = load_registry(&traces_path);
    let updated = {
        let entry = registry
            .iter_mut()
            .find(|t| t.id == trace_id)
            .ok_or_else(|| format!("Trace introuvable : {}.", trace_id))?;
        entry.stats = stats;
        entry.hash = hash;
        entry.audit_status = "clean".to_string();
        // L'audit appliqué est archivé dans le dossier de la trace : la carte du
        // circuit peut alors proposer la consultation des anomalies.
        entry.audit_archived = true;
        // Le parcours se poursuit par la validation des passages multiples.
        entry.multiride_status = multiride_status;
        entry.clone()
    };
    save_registry(&traces_path, &registry)?;

    Ok(updated)
}

/// Compteurs du bloc d'audit écrit dans le GPX (IHM §20.2).
pub fn findings_summary(findings: &[Finding]) -> FindingsSummary {
    FindingsSummary {
        routes: findings
            .iter()
            .filter(|f| {
                matches!(
                    f.correction,
                    Some(CorrectionType::RouteCar) | Some(CorrectionType::RouteBike)
                )
            })
            .count(),
        deletions: findings
            .iter()
            .filter(|f| matches!(f.correction, Some(CorrectionType::Delete)))
            .count(),
        false_positives: findings
            .iter()
            .filter(|f| f.status == FindingStatus::Fp)
            .count(),
    }
}

// ─── 11. Archive d'audit ──────────────────────────────────────────────

/// Écrit l'archive d'audit d'une trace (état de travail courant).
///
/// Appelée par le store **après la détection, puis après chaque traitement**
/// (suppression, routage, faux positif, annulation) : le travail survit ainsi à
/// la sortie de la vue et au redémarrage de l'application, et peut être
/// restitué — reprise d'une session `needs_review`, consultation d'un audit
/// `clean`.
///
/// Les **aperçus** (réglage continu des curseurs) ne passent jamais par ici :
/// seuls les traitements effectifs sont archivés. Retourne l'horodatage de
/// l'écriture, que le store publie comme date d'archivage.
#[tauri::command]
pub async fn audit_save_state(
    app: tauri::AppHandle,
    trace_id: String,
    params: AuditParams,
    total_distance_m: f64,
    points: Vec<AuditPoint>,
    findings: Vec<Finding>,
    validated: bool,
) -> Result<String, String> {
    let mode_dir = get_mode_dir(&app)?;
    save_state_impl(
        &mode_dir,
        &trace_id,
        params,
        total_distance_m,
        points,
        findings,
        validated,
    )
}

/// Implémentation testable de `audit_save_state` (sans `AppHandle`).
///
/// Retourne l'horodatage de l'archive écrite.
pub fn save_state_impl(
    mode_dir: &Path,
    trace_id: &str,
    params: AuditParams,
    total_distance_m: f64,
    points: Vec<AuditPoint>,
    findings: Vec<Finding>,
    validated: bool,
) -> Result<String, String> {
    let archive = archive::build_archive(
        trace_id,
        validated,
        params,
        total_distance_m,
        points,
        findings,
    );
    let updated_at = archive.updated_at.clone();
    archive::save_archive(&archive::archive_path(mode_dir, trace_id), &archive)?;
    println!(
        "[audit] archive trace={} points={} findings={} validée={}",
        trace_id,
        archive.points.len(),
        archive.findings.len(),
        validated
    );
    Ok(updated_at)
}

/// Lit l'archive d'audit d'une trace (`null` si absente ou inexploitable).
///
/// Une archive illisible ne remonte jamais d'erreur : le module retombe sur la
/// détection, qui reste la source de vérité.
#[tauri::command]
pub async fn audit_load_archive(
    app: tauri::AppHandle,
    trace_id: String,
) -> Result<Option<AuditArchive>, String> {
    let mode_dir = get_mode_dir(&app)?;
    println!("[audit] lecture archive trace={}", trace_id);
    Ok(load_archive_impl(&mode_dir, &trace_id))
}

/// Implémentation testable de `audit_load_archive` (sans `AppHandle`).
pub fn load_archive_impl(mode_dir: &Path, trace_id: &str) -> Option<AuditArchive> {
    archive::load_archive(&archive::archive_path(mode_dir, trace_id), trace_id)
}
