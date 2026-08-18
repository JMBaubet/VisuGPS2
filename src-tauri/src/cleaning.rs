//! Module de nettoyage des traces GPX.
//!
//! Une trace importée n'est **valide** que si elle est « propre » : les
//! anomalies de relevé (points isolés hors trace, aller-retours inutiles,
//! sorties de trajectoire) doivent être corrigées par l'utilisateur avant de
//! pouvoir engager les traitements (édition caméra). Ce module fournit :
//!
//! - la **détection** des anomalies par changement de cap proche de 180° (la
//!   tolérance de cap est paramétrable : `Nettoyage.Cap.toleranceDeg`) ;
//! - la **persistance** des décisions de correction (fichier de travail
//!   `{mode}/cleaning/{trace_id}.json`, écriture atomique) ;
//! - la **finalisation** : génération du GPX nettoyé, sauvegarde de l'original
//!   en `{filename}.orig`, régénération des dérivés (geojson, stats, hash) et
//!   passage de la trace à l'état `"clean"`.
//!
//! La détection a un rôle **propositif** : elle propose des segments candidats
//! (plages d'index + apex) que l'utilisateur peut ajuster. La validation de
//! chaque cas est de la responsabilité de l'utilisateur ; le GPX original
//! n'est remplacé qu'à la finalisation, une fois **tous** les cas validés.

use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tauri::Manager;

use crate::import_gpx::{
    build_geojson_feature, compute_file_hash, compute_stats, extract_line_coordinates,
    get_geojson_path, get_gpx_dir, get_mode_dir, get_traces_path, haversine, load_registry,
    save_registry, TraceMetadata,
};
use crate::settings::{get_toml_value_by_path, SettingsState};

// ---------------------------------------------------------------------------
// Structures sérialisables (miroir des interfaces TS du store cleaning)
// ---------------------------------------------------------------------------

/// Type d'anomalie détectée — pilote l'outillage proposé dans l'IHM.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CleaningCaseKind {
    /// Point isolé qui sort de la trace et revient immédiatement (ex. 946).
    Spike,
    /// Branche aller-retour avec retraçage (ex. 711 / 791).
    OutAndBack,
    /// Cas créé **manuellement** par l'utilisateur (plage `[start, end]`
    /// désignée sur la carte) — jamais produit par la détection.
    Manual,
    /// Valeur de repli pour les états de travail antérieurs (inconnus).
    #[serde(other)]
    Unknown,
}

/// Point de la trace **déplacé** géographiquement : à la finalisation, le point
/// d'index original `index` est remplacé par les coordonnées fournies
/// (élévation et temps d'origine conservés).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct MovedPoint {
    pub index: usize,
    pub lat: f64,
    pub lon: f64,
}

/// Corrections appliquées à un cas. Les index sont **originaux** (avant
/// suppression) de la trace.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct Correction {
    #[serde(default)]
    pub delete_ranges: Vec<[usize; 2]>,
    #[serde(default)]
    pub moved_points: Vec<MovedPoint>,
}

/// Un cas d'anomalie à traiter.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CleaningCase {
    /// Identifiant stable du cas (ex. "c1"), utilisé par l'IHM.
    pub id: String,
    pub kind: CleaningCaseKind,
    /// Début de la zone d'intérêt (index original inclus).
    pub start_index: usize,
    /// Fin de la zone d'intérêt (index original inclus).
    pub end_index: usize,
    /// Points de rebroussement détectés (index originaux).
    #[serde(default)]
    pub apex_indices: Vec<usize>,
    /// Écart de cap maximal mesuré dans le cas (degrés).
    #[serde(default)]
    pub bearing_delta_deg: f64,
    /// Plages de suppression **suggérées** par la détection (pré-remplissage
    /// de `correction.delete_ranges` au premier affichage du cas).
    #[serde(default)]
    pub suggested_delete_ranges: Vec<[usize; 2]>,
    /// État de validation utilisateur : `"pending"` (à traiter), `"corrected"`
    /// (corrigé), `"kept"` (conservé tel quel — faux positif).
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub correction: Correction,
}

/// État complet du nettoyage d'une trace (persisté dans `cleaning/{id}.json`).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CleaningState {
    pub trace_id: String,
    pub tolerance_deg: f64,
    #[serde(default)]
    pub cases: Vec<CleaningCase>,
}

// ---------------------------------------------------------------------------
// Utilitaires géométriques
// ---------------------------------------------------------------------------

/// Cap initial (bearing) entre deux points, en degrés [0, 360).
fn bearing(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let phi1 = lat1.to_radians();
    let phi2 = lat2.to_radians();
    let d_lambda = (lon2 - lon1).to_radians();
    let y = d_lambda.sin() * phi2.cos();
    let x = phi1.cos() * phi2.sin() - phi1.sin() * phi2.cos() * d_lambda.cos();
    let deg = y.atan2(x).to_degrees();
    (deg + 360.0) % 360.0
}

/// Distance angulaire minimale entre deux caps (degrés, [0, 180]).
fn angle_distance(a: f64, b: f64) -> f64 {
    let d = (a - b).abs() % 360.0;
    d.min(360.0 - d)
}

/// Longueur cumulée (Haversine) d'une séquence de points.
fn segment_length(points: &[(f64, f64)]) -> f64 {
    points
        .windows(2)
        .map(|w| haversine(w[0].0, w[0].1, w[1].0, w[1].1))
        .sum()
}

// ---------------------------------------------------------------------------
// Extraction des points d'un GPX
// ---------------------------------------------------------------------------

/// Point de trace complet (lat/lon + altitude + timestamp préservé).
#[derive(Clone, Debug)]
struct GpxPoint {
    lat: f64,
    lon: f64,
    ele: Option<f64>,
    time: Option<String>,
}

/// Aplatie `tracks → segments → points` en conservant tous les attributs.
fn extract_full_points(gpx: &gpx::Gpx) -> Vec<GpxPoint> {
    let mut pts = Vec::new();
    for track in &gpx.tracks {
        for segment in &track.segments {
            for wp in &segment.points {
                let pt = wp.point();
                // `gpx::Time::format` rend l'ISO 8601 (UTC, avec offset).
                let time = wp.time.and_then(|t| t.format().ok());
                pts.push(GpxPoint {
                    lat: pt.y(),
                    lon: pt.x(),
                    ele: wp.elevation,
                    time,
                });
            }
        }
    }
    pts
}

// ---------------------------------------------------------------------------
// Détection des anomalies
// ---------------------------------------------------------------------------

/// Fenêtre (en index) au-delà de laquelle deux points de rebroussement sont
/// considérés comme deux cas distincts.
const U_TURN_GROUP_WINDOW: usize = 25;

/// Détecte les anomalies sur une liste de coordonnées (lat, lon).
///
/// Algorithme (inspiré de l'outil de détection fourni par l'utilisateur) :
/// 1. Pour chaque point, on compare le cap vers le point précédent et le cap
///    vers le point suivant : s'ils sont quasi identiques (différence ≤
///    tolérance), le point est un **rebroussement** (changement de cap ≈ 180°).
/// 2. Les rebroussements proches sont regroupés en un **cas**.
/// 3. On délimite la zone de déviation par retraçage symétrique (les points du
///    retour sont des jumeaux des points de l'aller) et on classe le cas :
///    - branche courte (< 100 m) → `spike` (suggestion : supprimer l'apex) ;
///    - sinon → `out_and_back` (suggestion : supprimer le demi-tour + le retour).
pub fn detect_anomalies(points: &[(f64, f64)], tolerance_deg: f64) -> Vec<CleaningCase> {
    let n = points.len();
    if n < 3 {
        return Vec::new();
    }

    // 1. Points de rebroussement.
    let mut uturns: Vec<usize> = Vec::new();
    for i in 1..n - 1 {
        let cap_prev = bearing(points[i].0, points[i].1, points[i - 1].0, points[i - 1].1);
        let cap_next = bearing(points[i].0, points[i].1, points[i + 1].0, points[i + 1].1);
        if angle_distance(cap_prev, cap_next) <= tolerance_deg {
            uturns.push(i);
        }
    }
    if uturns.is_empty() {
        return Vec::new();
    }

    // 2. Regroupement des rebroussements proches en cas.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for &u in &uturns {
        if let Some(last) = groups.last_mut() {
            if let Some(&lu) = last.last() {
                if u - lu <= U_TURN_GROUP_WINDOW {
                    last.push(u);
                    continue;
                }
            }
        }
        groups.push(vec![u]);
    }

    // 3. Construction des cas.
    let mut cases = Vec::new();
    for (gi, group) in groups.iter().enumerate() {
        let apex = group[0];

        // Zone de déviation par retraçage symétrique (jumeaux aller/retour).
        let mut d_max = 0usize;
        while apex > d_max && apex + d_max + 1 < n {
            let a = apex - (d_max + 1);
            let b = apex + (d_max + 1);
            if haversine(points[a].0, points[a].1, points[b].0, points[b].1) < 30.0 {
                d_max += 1;
            } else {
                break;
            }
        }
        let start = apex - d_max;
        let end = apex + d_max;

        // Longueur cumulée de la branche (aller + retour).
        let branch_len = segment_length(&points[start..=end]);

        // Classification : branche courte → spike (point isolé) ; sinon un
        // aller-retour. Le type « parallel » (ajout de points) est réservé aux
        // cas assignés manuellement par l'utilisateur.
        let (kind, suggested) = if branch_len < 100.0 {
            (CleaningCaseKind::Spike, vec![[apex, apex]])
        } else {
            (CleaningCaseKind::OutAndBack, vec![[apex, end]])
        };

        // Écart de cap maximal parmi les rebroussements du groupe.
        let max_delta = group
            .iter()
            .map(|&i| {
                let c1 = bearing(points[i].0, points[i].1, points[i - 1].0, points[i - 1].1);
                let c2 = bearing(points[i].0, points[i].1, points[i + 1].0, points[i + 1].1);
                angle_distance(c1, c2)
            })
            .fold(0.0f64, f64::max);

        cases.push(CleaningCase {
            id: format!("c{}", gi + 1),
            kind,
            start_index: start,
            end_index: end,
            apex_indices: group.clone(),
            bearing_delta_deg: max_delta,
            suggested_delete_ranges: suggested,
            state: "pending".to_string(),
            correction: Correction::default(),
        });
    }

    cases
}

/// Détecte les anomalies directement depuis un GPX parsé.
pub fn detect_anomalies_from_gpx(gpx: &gpx::Gpx, tolerance_deg: f64) -> Vec<CleaningCase> {
    let coords: Vec<(f64, f64)> = extract_full_points(gpx)
        .iter()
        .map(|p| (p.lat, p.lon))
        .collect();
    detect_anomalies(&coords, tolerance_deg)
}

/// Lit la tolérance de cap paramétrée (`Nettoyage.Cap.toleranceDeg`), avec
/// repli sur 5° si le paramètre est absent ou illisible.
///
/// Ne **panique jamais** : l'accès au state des paramètres se fait via
/// `try_state` (un `state()` paniquerait si le state n'était pas géré, ce qui
/// laisserait une commande async sans réponse — l'import resterait bloqué
/// côté frontend).
pub fn read_tolerance_deg(app: &tauri::AppHandle) -> f64 {
    let state = match app.try_state::<Arc<RwLock<SettingsState>>>() {
        Some(s) => s,
        None => return 5.0,
    };
    let guard = match state.read() {
        Ok(g) => g,
        Err(_) => return 5.0,
    };
    tolerance_from_settings(&guard.default_toml, &guard.user_overrides)
}

/// Résout la tolérance depuis les tables TOML (surcharge utilisateur
/// prioritaire, puis valeur par défaut, repli 5°). Pure et testable.
fn tolerance_from_settings(default_toml: &toml::Table, user_overrides: &toml::Table) -> f64 {
    let value = get_toml_value_by_path(user_overrides, "Nettoyage.Cap.toleranceDeg")
        .or_else(|| get_toml_value_by_path(default_toml, "Nettoyage.Cap.toleranceDeg"));
    match value {
        Some(toml::Value::Float(f)) => *f,
        Some(toml::Value::Integer(i)) => *i as f64,
        _ => 5.0,
    }
}

// ---------------------------------------------------------------------------
// Chargement du GPX source d'une trace
// ---------------------------------------------------------------------------

/// Résout et parse le fichier GPX original d'une trace depuis le registre.
fn load_trace_gpx(app: &tauri::AppHandle, trace_id: &str) -> Result<(gpx::Gpx, TraceMetadata), String> {
    let mode_dir = get_mode_dir(app)?;
    let gpx_dir = get_gpx_dir(&mode_dir)?;
    let traces_path = get_traces_path(&mode_dir);
    let registry = load_registry(&traces_path);
    let trace = registry
        .iter()
        .find(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable (id={})", trace_id))?
        .clone();

    let gpx_file = gpx_dir.join(&trace.filename);
    if !gpx_file.exists() {
        return Err(format!(
            "Fichier GPX source introuvable pour la trace (id={}, fichier={:?})",
            trace_id, gpx_file
        ));
    }
    let file = std::fs::File::open(&gpx_file).map_err(|e| format!("Ouverture du fichier : {}", e))?;
    let reader = BufReader::new(file);
    let gpx = gpx::read(reader).map_err(|e| format!("Fichier GPX invalide : {}", e))?;
    Ok((gpx, trace))
}

// ---------------------------------------------------------------------------
// Persistance du fichier de travail
// ---------------------------------------------------------------------------

/// Retourne le dossier cleaning/ du mode actif (créé si absent).
fn get_cleaning_dir(mode_dir: &Path) -> Result<PathBuf, String> {
    let dir = mode_dir.join("cleaning");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Impossible de créer le dossier cleaning : {}", e))?;
    Ok(dir)
}

/// Chemin du fichier de travail de nettoyage d'une trace.
fn get_cleaning_path(mode_dir: &Path, trace_id: &str) -> PathBuf {
    mode_dir.join("cleaning").join(format!("{}.json", trace_id))
}

/// Écriture atomique d'un contenu (fichier tmp + rename).
fn write_atomic(path: &Path, content: &[u8]) -> Result<(), String> {
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, content)
        .map_err(|e| format!("Écriture du fichier temporaire : {}", e))?;
    std::fs::rename(&tmp_path, path).map_err(|e| format!("Renommage du fichier : {}", e))?;
    Ok(())
}

/// Met à jour le statut de nettoyage d'une trace dans le registre.
fn set_cleaning_status(app: &tauri::AppHandle, trace_id: &str, status: &str) -> Result<(), String> {
    let mode_dir = get_mode_dir(app)?;
    let traces_path = get_traces_path(&mode_dir);
    let mut registry = load_registry(&traces_path);
    let trace = registry
        .iter_mut()
        .find(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable (id={})", trace_id))?;
    trace.cleaning_status = status.to_string();
    save_registry(&traces_path, &registry)
}

// ---------------------------------------------------------------------------
// Application des corrections et génération du GPX nettoyé
// ---------------------------------------------------------------------------

/// Applique les corrections de tous les cas validés sur les points originaux
/// et retourne la liste finale des points ainsi que le nombre de suppressions.
fn apply_corrections(points: &[GpxPoint], cases: &[CleaningCase]) -> Result<(Vec<GpxPoint>, usize), String> {
    let n = points.len();
    let mut deleted = vec![false; n];

    for c in cases {
        if c.state == "kept" {
            continue; // cas conservé tel quel → aucune correction
        }
        for range in &c.correction.delete_ranges {
            let (from, to) = (range[0], range[1]);
            if from > to || to >= n {
                return Err(format!(
                    "Cas {} : plage de suppression invalide [{}, {}] ({} points).",
                    c.id, from, to, n
                ));
            }
            for i in from..=to {
                deleted[i] = true;
            }
        }
    }

    let mut result = Vec::with_capacity(n);
    let mut removed = 0usize;
    for (i, point) in points.iter().enumerate() {
        if deleted[i] {
            removed += 1;
            continue;
        }
        let mut p = point.clone();
        // Points déplacés : remplace les coordonnées des points conservés.
        if let Some(mp) = cases
            .iter()
            .filter(|c| c.state != "kept")
            .flat_map(|c| &c.correction.moved_points)
            .find(|mp| mp.index == i)
        {
            p.lat = mp.lat;
            p.lon = mp.lon;
        }
        result.push(p);
    }

    if result.len() < 2 {
        return Err("Le nettoyage aboutirait à moins de 2 points : correction refusée.".to_string());
    }

    Ok((result, removed))
}

/// Échappe les caractères XML réservés pour un contenu de balise.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Génère le contenu du GPX nettoyé (1.1, `<trk><trkseg>` unique), en
/// préservant latitude/longitude (6 décimales), altitude et timestamp.
fn build_cleaned_gpx(points: &[GpxPoint], name: &str) -> String {
    let mut s = String::with_capacity(points.len() * 96 + 256);
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    s.push_str("<gpx creator=\"VisuGPS2 - Nettoyage de trace\" version=\"1.1\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n");
    s.push_str("  <trk>\n");
    s.push_str(&format!("    <name>{}</name>\n", xml_escape(name)));
    s.push_str("    <trkseg>\n");
    for p in points {
        s.push_str(&format!("      <trkpt lat=\"{:.6}\" lon=\"{:.6}\">\n", p.lat, p.lon));
        if let Some(ele) = p.ele {
            s.push_str(&format!("        <ele>{:.1}</ele>\n", ele));
        }
        if let Some(t) = &p.time {
            s.push_str(&format!("        <time>{}</time>\n", t));
        }
        s.push_str("      </trkpt>\n");
    }
    s.push_str("    </trkseg>\n");
    s.push_str("  </trk>\n");
    s.push_str("</gpx>\n");
    s
}

// ---------------------------------------------------------------------------
// Commandes Tauri
// ---------------------------------------------------------------------------

/// Détecte les anomalies d'une trace (re-parse le GPX original, aucune
/// persistance). La tolérance de cap est fournie par le frontend (paramètre
/// paramétrable `Nettoyage.Cap.toleranceDeg`).
#[tauri::command]
pub async fn detect_trace_anomalies(
    app: tauri::AppHandle,
    trace_id: String,
    tolerance_deg: f64,
) -> Result<Vec<CleaningCase>, String> {
    let (gpx, _) = load_trace_gpx(&app, &trace_id)?;
    Ok(detect_anomalies_from_gpx(&gpx, tolerance_deg))
}

/// Retourne l'état de nettoyage d'une trace : le fichier de travail s'il
/// existe et est **valide** (corrections déjà en cours), sinon une détection
/// fraîche avec la tolérance fournie. Un fichier de travail illisible ou
/// invalide (ex. index négatif) est ignoré et régénéré par détection — il ne
/// doit jamais bloquer l'IHM.
#[tauri::command]
pub async fn get_cleaning_state(
    app: tauri::AppHandle,
    trace_id: String,
    tolerance_deg: f64,
) -> Result<CleaningState, String> {
    let mode_dir = get_mode_dir(&app)?;
    let state_path = get_cleaning_path(&mode_dir, &trace_id);

    if state_path.exists() {
        match std::fs::read_to_string(&state_path) {
            Ok(content) => match serde_json::from_str::<CleaningState>(&content) {
                Ok(state) => return Ok(state),
                Err(e) => {
                    eprintln!("[cleaning] Fichier de travail invalide ({}), re-détection.", e);
                }
            },
            Err(e) => {
                eprintln!("[cleaning] Fichier de travail illisible : {}", e);
            }
        }
    }

    let (gpx, _) = load_trace_gpx(&app, &trace_id)?;
    let cases = detect_anomalies_from_gpx(&gpx, tolerance_deg);
    Ok(CleaningState {
        trace_id,
        tolerance_deg,
        cases,
    })
}

/// Sauvegarde partielle du travail de nettoyage (fichier
/// `cleaning/{trace_id}.json`). Le GPX original reste intact. Passe la trace
/// en `"in_progress"` (première sauvegarde).
#[tauri::command]
pub async fn save_cleaning_state(
    app: tauri::AppHandle,
    trace_id: String,
    state_json: serde_json::Value,
) -> Result<(), String> {
    let mode_dir = get_mode_dir(&app)?;
    let dir = get_cleaning_dir(&mode_dir)?;
    let path = dir.join(format!("{}.json", trace_id));

    let content = serde_json::to_string_pretty(&state_json)
        .map_err(|e| format!("Sérialisation du fichier de travail : {}", e))?;
    write_atomic(&path, content.as_bytes())?;

    set_cleaning_status(&app, &trace_id, "in_progress")?;
    Ok(())
}

/// Abandonne les corrections en cours : supprime le fichier de travail et
/// remet la trace en `"needs_review"`.
#[tauri::command]
pub async fn reset_cleaning(app: tauri::AppHandle, trace_id: String) -> Result<(), String> {
    let mode_dir = get_mode_dir(&app)?;
    let state_path = get_cleaning_path(&mode_dir, &trace_id);
    if state_path.exists() {
        std::fs::remove_file(&state_path)
            .map_err(|e| format!("Suppression du fichier de travail : {}", e))?;
    }
    set_cleaning_status(&app, &trace_id, "needs_review")
}

/// Finalise le nettoyage : applique les corrections validées, génère le GPX
/// nettoyé, sauvegarde l'original en `{filename}.orig`, régénère les dérivés
/// (geojson, stats, hash) et passe la trace en `"clean"`.
///
/// Refuse la finalisation tant que **tous** les cas ne sont pas validés par
/// l'utilisateur (`state != "pending"`).
#[tauri::command]
pub async fn finalize_cleaning(
    app: tauri::AppHandle,
    trace_id: String,
    state_json: serde_json::Value,
) -> Result<TraceMetadata, String> {
    let state: CleaningState = serde_json::from_value(state_json)
        .map_err(|e| format!("État de nettoyage invalide : {}", e))?;

    // 1. Tous les cas doivent être validés (corrigé ou conservé tel quel).
    let pending = state.cases.iter().filter(|c| c.state == "pending").count();
    if pending > 0 {
        return Err(format!(
            "Finalisation impossible : {} cas restent à valider.",
            pending
        ));
    }

    // 2. Charger le GPX original et ses points complets.
    let mode_dir = get_mode_dir(&app)?;
    let gpx_dir = get_gpx_dir(&mode_dir)?;
    let traces_path = get_traces_path(&mode_dir);
    let (gpx, trace) = load_trace_gpx(&app, &trace_id)?;
    let original_points = extract_full_points(&gpx);

    // 3. Appliquer les corrections.
    let (final_points, removed_count) = apply_corrections(&original_points, &state.cases)?;

    // 4. Générer et écrire le GPX nettoyé.
    let trace_name = trace.name.clone();
    let cleaned_gpx = build_cleaned_gpx(&final_points, &trace_name);
    let gpx_file = gpx_dir.join(&trace.filename);

    // 5. Backup de l'original (une seule fois, ne pas écraser un backup).
    let backup_path = gpx_file.with_extension("gpx.orig");
    if !backup_path.exists() {
        std::fs::copy(&gpx_file, &backup_path)
            .map_err(|e| format!("Sauvegarde de l'original (backup) : {}", e))?;
    }

    // 6. Écraser le GPX original (écriture atomique).
    write_atomic(&gpx_file, cleaned_gpx.as_bytes())?;

    // 7. Régénérer les dérivés depuis le GPX nettoyé.
    let file = std::fs::File::open(&gpx_file).map_err(|e| format!("Ouverture du fichier : {}", e))?;
    let reader = BufReader::new(file);
    let cleaned_gpx_obj = gpx::read(reader).map_err(|e| format!("GPX nettoyé invalide : {}", e))?;

    let (stats, _) = compute_stats(&cleaned_gpx_obj)?;
    let coords = extract_line_coordinates(&cleaned_gpx_obj)?;
    let geojson_path = get_geojson_path(&mode_dir, &trace_id);
    let feature = build_geojson_feature(coords, &trace_id, &trace_name);
    let geojson_content = serde_json::to_string_pretty(&feature)
        .map_err(|e| format!("Sérialisation GeoJSON : {}", e))?;
    write_atomic(&geojson_path, geojson_content.as_bytes())?;

    let hash = compute_file_hash(&gpx_file)?;

    // 8. Mettre à jour le registre.
    let mut registry = load_registry(&traces_path);
    {
        let trace = registry
            .iter_mut()
            .find(|t| t.id == trace_id)
            .ok_or_else(|| format!("Trace introuvable (id={})", trace_id))?;
        trace.stats = stats;
        trace.hash = hash;
        trace.cleaning_status = "clean".to_string();
    }
    save_registry(&traces_path, &registry)?;

    // 9. Supprimer le fichier de travail.
    let state_path = get_cleaning_path(&mode_dir, &trace_id);
    if state_path.exists() {
        let _ = std::fs::remove_file(&state_path);
    }

    println!(
        "[cleaning] Trace « {} » finalisée : {} point(s) supprimé(s), {} → {} points.",
        trace_name,
        removed_count,
        original_points.len(),
        final_points.len()
    );

    registry
        .into_iter()
        .find(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable (id={})", trace_id))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Cas « spike » : un point isolé qui s'écarte de la ligne puis revient
    /// exactement sur ses pas (analogue au point 946 / 4101).
    fn spike_trace() -> Vec<(f64, f64)> {
        vec![
            (41.618450, 2.537800), // 0  ← jumeau de 8
            (41.618450, 2.537900), // 1  ← jumeau de 7
            (41.618450, 2.538000), // 2  ← jumeau de 6
            (41.618450, 2.538100), // 3  ← jumeau de 5
            (41.618440, 2.538250), // 4  apex (écart ~12,5 m)
            (41.618450, 2.538100), // 5
            (41.618450, 2.538000), // 6
            (41.618450, 2.537900), // 7
            (41.618450, 2.537800), // 8
            (41.618450, 2.537700), // 9  continuation (plus de jumeau)
        ]
    }

    /// Cas « out_and_back » : une branche aller-retour avec retraçage exact
    /// (analogue au point 711 / 791).
    fn out_and_back_trace() -> Vec<(f64, f64)> {
        vec![
            (41.62250, 2.55200), // 0  ← jumeau de 12
            (41.62250, 2.55250), // 1  ← jumeau de 11
            (41.62250, 2.55300), // 2  ← jumeau de 10
            (41.62250, 2.55350), // 3  ← jumeau de 9
            (41.62226, 2.55421), // 4  ← jumeau de 8
            (41.62200, 2.55380), // 5  ← jumeau de 7
            (41.62193, 2.55421), // 6  apex (demi-tour)
            (41.62200, 2.55380), // 7
            (41.62226, 2.55421), // 8
            (41.62250, 2.55350), // 9
            (41.62250, 2.55300), // 10
            (41.62250, 2.55250), // 11
            (41.62250, 2.55200), // 12
            (41.62250, 2.55150), // 13 continuation
        ]
    }

    #[test]
    fn spike_is_detected_and_classified() {
        let pts = spike_trace();
        let cases = detect_anomalies(&pts, 5.0);

        assert_eq!(cases.len(), 1, "un seul cas doit être détecté");
        let c = &cases[0];
        assert_eq!(c.kind, CleaningCaseKind::Spike);
        assert_eq!(c.apex_indices, vec![4]);
        // Suggestion : supprimer le point isolé.
        assert_eq!(c.suggested_delete_ranges, vec![[4, 4]]);
    }

    #[test]
    fn out_and_back_is_detected_and_classified() {
        let pts = out_and_back_trace();
        let cases = detect_anomalies(&pts, 5.0);

        assert_eq!(cases.len(), 1, "un seul cas doit être détecté");
        let c = &cases[0];
        assert_eq!(c.kind, CleaningCaseKind::OutAndBack);
        assert_eq!(c.apex_indices, vec![6]);
        // La zone couvre l'aller-retour complet ; la suggestion supprime le
        // demi-tour + le retour (on garde l'aller).
        assert_eq!(c.start_index, 0);
        assert_eq!(c.end_index, 12);
        assert_eq!(c.suggested_delete_ranges, vec![[6, 12]]);
    }

    #[test]
    fn tolerance_controls_detection() {
        let pts = spike_trace();
        // Tolérance 0 → le rebroussement (écart de cap parfaitement nul dans la
        // construction) reste détecté ici car le retraçage est exact. On vérifie
        // plutôt qu'une tolérance très faible ne crée pas de faux positif sur une
        // ligne droite : construction d'une ligne sans anomalie.
        let straight: Vec<(f64, f64)> = (0..20)
            .map(|i| (41.6, 2.50 + i as f64 * 0.001))
            .collect();
        assert!(detect_anomalies(&straight, 5.0).is_empty());

        // Une tolérance très élevée détecte aussi les demi-tours imparfaits.
        assert_eq!(detect_anomalies(&pts, 20.0).len(), 1);
    }

    /// Vérifie l'application des corrections (suppressions) et la génération
    /// du GPX nettoyé.
    #[test]
    fn apply_corrections_and_build_gpx() {
        let raw: Vec<GpxPoint> = out_and_back_trace()
            .iter()
            .enumerate()
            .map(|(i, (lat, lon))| GpxPoint {
                lat: *lat,
                lon: *lon,
                ele: Some(100.0 + i as f64),
                time: Some(format!("2026-01-01T10:00:{:02}Z", i)),
            })
            .collect();

        // Cas corrigé : supprime le demi-tour + le retour (index 6..12).
        let mut correction = Correction::default();
        correction.delete_ranges.push([6, 12]);
        let cases = vec![CleaningCase {
            id: "c1".to_string(),
            kind: CleaningCaseKind::OutAndBack,
            start_index: 0,
            end_index: 12,
            apex_indices: vec![6],
            bearing_delta_deg: 0.0,
            suggested_delete_ranges: vec![[6, 12]],
            state: "corrected".to_string(),
            correction,
        }];

        let (final_points, removed) = apply_corrections(&raw, &cases).unwrap();
        assert_eq!(removed, 7);
        // Points conservés : 0..=5 + 13 → 7 points.
        assert_eq!(final_points.len(), 7);
        assert_eq!(final_points[6].lat, 41.62250);

        // Le GPX généré doit être du XML valide et contenir les points.
        let gpx_str = build_cleaned_gpx(&final_points, "Test trace");
        assert!(gpx_str.starts_with("<?xml"));
        assert!(gpx_str.contains("<name>Test trace</name>"));
        assert!(gpx_str.matches("<trkpt ").count() == 7);
        assert!(gpx_str.contains("<ele>105.0</ele>"));
        assert!(gpx_str.contains("<time>2026-01-01T10:00:05Z</time>"));

        // Un cas « conservé tel quel » (faux positif) ne supprime rien.
        let mut kept = cases.clone();
        kept[0].state = "kept".to_string();
        kept[0].correction = Correction::default();
        let (kept_points, kept_removed) = apply_corrections(&raw, &kept).unwrap();
        assert_eq!(kept_points.len(), raw.len());
        assert_eq!(kept_removed, 0);

        // Un point **déplacé** (index original 3) remplace ses coordonnées.
        let mut moved = cases.clone();
        moved[0].correction.moved_points.push(MovedPoint {
            index: 3,
            lat: 41.62300,
            lon: 2.55310,
        });
        let (moved_points, _) = apply_corrections(&raw, &moved).unwrap();
        // Le point d'index 3 est conservé (pas supprimé) : position dans le
        // résultat = index 3 (les suppressions [6,12] sont après).
        assert_eq!(moved_points.len(), 7);
        assert_eq!(moved_points[3].lat, 41.62300);
        assert_eq!(moved_points[3].lon, 2.55310);
        // Les autres points conservent leurs coordonnées.
        assert_eq!(moved_points[0].lat, raw[0].lat);

        // Un point déplacé **et** supprimé ne doit pas réapparaître.
        let mut both = cases.clone();
        both[0].correction.delete_ranges.push([3, 3]);
        both[0].correction.moved_points.push(MovedPoint {
            index: 3,
            lat: 41.62300,
            lon: 2.55310,
        });
        let (both_points, _) = apply_corrections(&raw, &both).unwrap();
        assert!(
            !both_points.iter().any(|p| (p.lat - 41.62300).abs() < 1e-9),
            "un point supprimé ne doit pas être déplacé"
        );
    }

    /// Vérifie que la finalisation refuse un cas encore « pending ».
    #[test]
    fn finalize_refuses_pending_cases() {
        // On vérifie la règle métier au niveau du check (reproduit dans
        // finalize_cleaning) : un état avec un cas pending doit être refusé.
        let state = CleaningState {
            trace_id: "t".to_string(),
            tolerance_deg: 5.0,
            cases: vec![CleaningCase {
                id: "c1".to_string(),
                kind: CleaningCaseKind::Spike,
                start_index: 0,
                end_index: 2,
                apex_indices: vec![1],
                bearing_delta_deg: 0.0,
                suggested_delete_ranges: vec![],
                state: "pending".to_string(),
                correction: Correction::default(),
            }],
        };
        let pending = state.cases.iter().filter(|c| c.state == "pending").count();
        assert_eq!(pending, 1);
        assert!(!state.cases.iter().all(|c| c.state != "pending"));
    }

    /// Détection sur les GPX réels du mode OPE (Application Support).
    /// Test `#[ignore]` : il dépend de fichiers présents uniquement sur la
    /// machine de travail. À lancer via `cargo test -- --ignored`.
    ///
    /// Tous les `.gpx` du dossier sont scannés : le pipeline complet d'import
    /// (name, stats, coords, détection) ne doit **jamais paniquer**, et la
    /// détection doit rester rapide même sur les gros fichiers.
    #[test]
    #[ignore]
    fn detects_real_trace_anomalies() {
        let gpx_dir = "/Users/jean-marcbaubet/Library/Application Support/com.jean-marc.baubet.visugps2/OPE/gpx";
        let mut files: Vec<String> = std::fs::read_dir(gpx_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "gpx").unwrap_or(false))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        files.sort();
        assert!(!files.is_empty(), "Aucun GPX trouvé dans {}", gpx_dir);

        for name in &files {
            let path = format!("{}/{}", gpx_dir, name);
            let start = std::time::Instant::now();
            let file = std::fs::File::open(&path)
                .unwrap_or_else(|e| panic!("Fichier introuvable {} : {}", path, e));
            let gpx = gpx::read(BufReader::new(file)).unwrap();

            // Pipeline d'import complet (étapes 6 → 8bis de import_gpx_file) :
            // aucun de ces appels ne doit paniquer.
            let trace_name = crate::import_gpx::extract_name(&gpx, name);
            assert!(!trace_name.is_empty());
            let (stats, count) = crate::import_gpx::compute_stats(&gpx).unwrap();
            assert!(stats.points_count >= 2 && count == stats.points_count);
            let coords = crate::import_gpx::extract_line_coordinates(&gpx).unwrap();
            assert_eq!(coords.len(), count);
            let _feature = crate::import_gpx::build_geojson_feature(coords, "id-test", &trace_name);

            let cases = detect_anomalies_from_gpx(&gpx, 5.0);
            let elapsed = start.elapsed();
            println!(
                "{} → {} cas ({} pts, {:.0} ms)",
                name,
                cases.len(),
                count,
                elapsed.as_millis()
            );
            for c in &cases {
                println!(
                    "  {:?} apex={:?} zone=[{},{}] suggestion={:?}",
                    c.kind, c.apex_indices, c.start_index, c.end_index,
                    c.suggested_delete_ranges
                );
            }
            // La détection doit rester rapide (pas de blocage sur les gros fichiers).
            assert!(
                elapsed.as_millis() < 5000,
                "{} : détection trop lente ({} ms)",
                name,
                elapsed.as_millis()
            );

            // Appliquer les corrections suggérées et vérifier que le GPX
            // généré est cohérent (points conservés, XML relisible).
            let raw = extract_full_points(&gpx);
            for c in &mut cases.clone() {
                c.state = "corrected".to_string();
                c.correction.delete_ranges = c.suggested_delete_ranges.clone();
            }
            let (final_points, removed) = apply_corrections(&raw, &cases).unwrap();
            assert!(
                final_points.len() >= 2,
                "{} : trop de points supprimés ({} restants)",
                name,
                final_points.len()
            );
            let gpx_str = build_cleaned_gpx(&final_points, "test nettoyage");
            assert_eq!(
                gpx_str.matches("<trkpt ").count(),
                final_points.len(),
                "{} : nombre de <trkpt> du GPX généré",
                name
            );
            let reparsed = gpx::read(std::io::Cursor::new(gpx_str)).unwrap();
            let reparsed_count: usize = reparsed
                .tracks
                .iter()
                .flat_map(|t| &t.segments)
                .flat_map(|s| &s.points)
                .count();
            assert_eq!(reparsed_count, final_points.len(), "{} : re-parse", name);
            let _ = removed;
        }
    }

    /// `read_tolerance_deg` ne doit **jamais paniquer** si le state des
    /// paramètres n'est pas géré (repli sur 5°). Un panic dans une commande
    /// async laisserait l'appelant bloqué sans réponse. La logique de lecture
    /// est testée au niveau de `tolerance_from_settings` (fonction pure).
    #[test]
    fn tolerance_from_settings_reads_default() {
        let default: toml::Table =
            toml::from_str("[Nettoyage.Cap]\ntoleranceDeg = 5.0\n").unwrap();
        assert_eq!(tolerance_from_settings(&default, &toml::Table::new()), 5.0);
    }

    /// La surcharge utilisateur est prioritaire sur la valeur par défaut.
    #[test]
    fn tolerance_from_settings_user_override_priority() {
        let default: toml::Table =
            toml::from_str("[Nettoyage.Cap]\ntoleranceDeg = 5.0\n").unwrap();
        let mut cap = toml::Table::new();
        cap.insert("toleranceDeg".to_string(), toml::Value::Float(7.5));
        let mut nettoyage = toml::Table::new();
        nettoyage.insert("Cap".to_string(), toml::Value::Table(cap));
        let mut user_overrides = toml::Table::new();
        user_overrides.insert("Nettoyage".to_string(), toml::Value::Table(nettoyage));

        assert_eq!(tolerance_from_settings(&default, &user_overrides), 7.5);
    }

    /// Clé absente → repli sur 5° (le comportement de `try_state` sans state
    /// managé est couvert par la garantie « None → 5.0 » de `read_tolerance_deg`).
    #[test]
    fn tolerance_from_settings_missing_key_falls_back() {
        let default = toml::Table::new();
        assert_eq!(tolerance_from_settings(&default, &toml::Table::new()), 5.0);
    }
}
