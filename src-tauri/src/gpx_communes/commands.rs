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
use std::time::Duration;

use tauri::Manager;

use crate::hermes::client::{validate_id, HermesClient};
use crate::hermes::types::{
    coded, codes, HermesCommunesDetectRequest, HermesCommunesDetectResponse, HermesCommunesParams,
    HermesDataset, HermesPolygon, HermesRegion, HermesServingStatus,
};
use crate::import_gpx::{
    get_geojson_path, get_mode_dir, get_traces_path, load_registry, rebuild_geometry_from_gpx,
    update_registry,
};
use crate::settings::{get_toml_value_by_path, SettingsState};

use super::file;
use super::hermes::{coverage_rings, is_outside_coverage, passages_from_hermes};
use super::osm;
use super::pipeline;
use super::types::{CommuneDetection, CommuneParams, CommunePassage};

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

/// Message d'abandon commun aux deux moteurs : la trace a été réécrite (audit
/// appliqué) pendant que la détection travaillait sur l'ancienne géométrie.
const ABANDONED: &str = "Trace réécrite pendant la détection (audit appliqué) : la détection a été abandonnée et sera rejouée.";

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
/// sur la carte du circuit. Une trace **réécrite pendant l'analyse** (audit
/// appliqué) fait abandonner la détection : voir `commit_detection`.
pub fn detect_impl(
    mode_dir: &Path,
    trace_id: &str,
    osm_json: &str,
    params: &CommuneParams,
) -> Result<Vec<CommunePassage>, String> {
    // 1. Trace nettoyée : sa géométrie persistée, régénérée depuis le GPX si elle
    //    manque (trace importée avant ce cache), et l'empreinte qui servira de
    //    témoin pour détecter une réécriture pendant l'analyse.
    let trace = load_trace_coordinates(mode_dir, trace_id)?;
    let reference_hash = registry_hash(mode_dir, trace_id);

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

    // 4. Description et drapeau : l'ordre compte — une trace « détectée » sans
    //    description serait un état mensonger, et une trace supprimée ou
    //    réécrite pendant l'analyse ne doit rien recevoir.
    if commit_detection(mode_dir, trace_id, &passages, reference_hash.as_deref())? {
        return Err(ABANDONED.to_string());
    }

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

/// Empreinte du GPX d'une trace telle que le registre la connaît (`None` si la
/// trace n'y figure pas : l'appelant retombera sur « trace introuvable »).
fn registry_hash(mode_dir: &Path, trace_id: &str) -> Option<String> {
    load_registry(&get_traces_path(mode_dir))
        .into_iter()
        .find(|t| t.id == trace_id)
        .map(|t| t.hash)
}

/// Écrit la description des communes puis pose le drapeau, **sous le verrou du
/// registre** — le calcul, lui, est resté hors section critique.
///
/// Retourne `true` quand la détection est **abandonnée** : l'empreinte du GPX a
/// changé depuis le début de l'analyse, donc un audit a réécrit la trace et la
/// description qui vient d'être calculée décrit l'ancienne géométrie. Rien n'est
/// écrit dans ce cas : la trace reste « à détecter » et sera rejouée.
///
/// La comparaison porte sur l'empreinte — et non sur `audit_status`, qui vaut
/// `"clean"` **avant comme après** un audit appliqué, ou sur la date, qui ne dit
/// rien de la géométrie.
pub(crate) fn commit_detection(
    mode_dir: &Path,
    trace_id: &str,
    passages: &[CommunePassage],
    reference_hash: Option<&str>,
) -> Result<bool, String> {
    update_registry(&get_traces_path(mode_dir), |registry| {
        let entry = registry
            .iter_mut()
            .find(|t| t.id == trace_id)
            .ok_or_else(|| format!("Trace introuvable : {}.", trace_id))?;

        if reference_hash != Some(entry.hash.as_str()) {
            return Ok(true);
        }

        file::save_file(mode_dir, trace_id, passages)?;
        entry.communes_completed = true;
        Ok(false)
    })
}

// ─── Détection par hermes ─────────────────────────────────────────────

/// Détecte les communes traversées par une trace **via le serveur hermes** et
/// persiste le résultat — moteur nominal depuis l'intégration du serveur.
///
/// La trace n'est jamais transmise : la commande relit `trace.geojson` côté
/// backend et n'envoie au serveur que les **points** de la trace, avec les deux
/// réglages qui gardent un sens côté client (`adminLevel`, `minSegmentKm`).
/// L'écriture et le drapeau du registre passent par `commit_detection`, comme le
/// moteur Overpass : même garde d'empreinte, même fichier.
///
/// Le dataset vient de `GET /serving` — source de vérité unique de la sélection,
/// persistée côté serveur — et non du frontend, qui ne peut donc pas désigner un
/// dataset périmé.
#[tauri::command]
pub async fn communes_detect_hermes(
    app: tauri::AppHandle,
    trace_id: String,
) -> Result<CommuneDetection, String> {
    let mode_dir = get_mode_dir(&app)?;
    let params = read_communes_params(&app);
    let points = load_trace_coordinates(&mode_dir, &trace_id)?;
    let reference_hash = registry_hash(&mode_dir, &trace_id);

    let config = crate::hermes::config::read_config(&app).await;
    let client = HermesClient::new(&config)?;
    let dataset_id = serving_dataset(&client, config.request_timeout).await?;

    let request = build_detection_request(&trace_id, &dataset_id, points.clone(), &params);
    let response = match client
        .post::<HermesCommunesDetectResponse, _>(
            "/communes/detect",
            &request,
            config.request_timeout,
        )
        .await
    {
        Ok(response) => response,
        // 404 : dataset inconnu, ou `communes.geojson` absent. Le code réservé
        // `HERMES_NOT_PREPARED` permet au store de proposer la bonne action.
        Err(error) if error.starts_with(codes::NOT_FOUND) => {
            return Err(coded(
                codes::NOT_PREPARED,
                format!(
                    "Communes non préparées pour le dataset « {} » (communes.geojson absent). Contactez l'administrateur.",
                    dataset_id
                ),
            ));
        }
        Err(error) => return Err(error),
    };

    // Le contrôle de couverture ne sert qu'à départager un résultat **vide** :
    // une liste non vide prouve à elle seule que la trace est dans le dataset.
    let outside = response.passages.is_empty()
        && outside_coverage(&client, config.request_timeout, &dataset_id, &points).await;
    if outside {
        println!(
            "[communes] {} : aucun point dans la couverture du dataset {} — résultat vide consigné.",
            trace_id, dataset_id
        );
    }

    let detection =
        commit_hermes_detection(&mode_dir, &trace_id, &response, outside, reference_hash.as_deref())?;

    println!(
        "[communes] {} : {} passage(s) via hermes (dataset {}).",
        trace_id,
        detection.passages.len(),
        detection.dataset_id
    );
    Ok(detection)
}

/// Partie **locale** de `communes_detect_hermes`, testable sans réseau :
/// conversion de la réponse, écriture de la description et drapeau du registre.
///
/// L'ordre et la garde d'empreinte sont ceux du moteur Overpass : une trace
/// réécrite pendant l'analyse (audit appliqué) fait abandonner la détection,
/// rien n'est écrit, la trace sera reprise.
pub fn commit_hermes_detection(
    mode_dir: &Path,
    trace_id: &str,
    response: &HermesCommunesDetectResponse,
    outside_coverage: bool,
    reference_hash: Option<&str>,
) -> Result<CommuneDetection, String> {
    let passages = passages_from_hermes(&response.passages);
    if commit_detection(mode_dir, trace_id, &passages, reference_hash)? {
        return Err(ABANDONED.to_string());
    }
    Ok(CommuneDetection {
        passages,
        dataset_id: response.dataset_id.clone(),
        outside_coverage,
    })
}

/// Construit la requête de détection : identifiants, points de la trace et
/// réglages transmis au serveur. `adminLevel` vide est omis (le serveur applique
/// alors son mapping par pays).
pub fn build_detection_request(
    trace_id: &str,
    dataset_id: &str,
    points: Vec<[f64; 2]>,
    params: &CommuneParams,
) -> HermesCommunesDetectRequest {
    HermesCommunesDetectRequest {
        trace_id: trace_id.to_string(),
        dataset_id: dataset_id.to_string(),
        points,
        params: HermesCommunesParams {
            admin_level: Some(params.admin_level.clone())
                .filter(|level| !level.trim().is_empty()),
            min_segment_km: params.min_segment_km,
        },
    }
}

/// Dataset servi (`GET /serving`), validé avant de composer l'URL.
async fn serving_dataset(client: &HermesClient, timeout: Duration) -> Result<String, String> {
    let serving = client.get::<HermesServingStatus>("/serving", timeout).await?;
    let dataset_id = serving.dataset_id.ok_or_else(|| {
        coded(
            codes::NO_DATASET,
            "Aucun dataset servi par hermes. Sélectionnez un jeu de données dans la vue Hermes.",
        )
    })?;
    validate_id(&dataset_id, "dataset")?;
    Ok(dataset_id)
}

/// Vrai quand **aucun** point de la trace ne tombe dans la couverture du
/// dataset — le serveur renvoie alors `passages: []`, indistinguable d'un
/// « aucune commune traversée ».
///
/// Les deux requêtes de géométrie sont **facultatives** : un échec ne doit pas
/// transformer une détection réussie en erreur. Sans géométrie exploitable, on
/// ne conclut rien (`false`) — mieux vaut pas d'alerte qu'une alerte fausse.
async fn outside_coverage(
    client: &HermesClient,
    timeout: Duration,
    dataset_id: &str,
    points: &[[f64; 2]],
) -> bool {
    let dataset = match client
        .get::<HermesDataset>(&format!("/datasets/{}", dataset_id), timeout)
        .await
    {
        Ok(dataset) => dataset,
        Err(error) => {
            eprintln!("[communes] Couverture : dataset illisible ({}).", error);
            return false;
        }
    };

    let rings = if dataset
        .zones
        .as_deref()
        .is_some_and(|zones| !zones.is_empty())
    {
        match client
            .get::<Vec<HermesPolygon>>("/polygons", timeout)
            .await
        {
            Ok(polygons) => coverage_rings(&dataset, &[], &polygons),
            Err(error) => {
                eprintln!("[communes] Couverture : secteurs illisibles ({}).", error);
                return false;
            }
        }
    } else {
        match client
            .get::<Vec<HermesRegion>>("/regions", timeout)
            .await
        {
            Ok(regions) => coverage_rings(&dataset, &regions, &[]),
            Err(error) => {
                eprintln!("[communes] Couverture : régions illisibles ({}).", error);
                return false;
            }
        }
    };

    is_outside_coverage(points, &rings)
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
/// Tout se joue sous le verrou du registre : une trace inconnue ne change rien
/// (rien n'est supprimé), sinon la description est supprimée et le drapeau
/// reposé dans la même section critique — l'ordre inverse laisserait une trace
/// « détectée » sans description.
pub fn reset_impl(mode_dir: &Path, trace_id: &str) -> Result<(), String> {
    update_registry(&get_traces_path(mode_dir), |registry| {
        let entry = registry
            .iter_mut()
            .find(|t| t.id == trace_id)
            .ok_or_else(|| format!("Trace introuvable : {}.", trace_id))?;

        file::remove_file(mode_dir, trace_id);
        entry.communes_completed = false;
        Ok(())
    })?;

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
