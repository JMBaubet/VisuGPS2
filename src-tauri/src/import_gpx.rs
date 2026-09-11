//! Module d'import de fichiers GPX pour VisuGPS2.
//!
//! Ce module expose les commandes Tauri suivantes :
//! - `import_gpx_file` : ouvre un sélecteur natif, parse le fichier, calcule les stats,
//!   stocke le fichier et les métadonnées dans le dossier du mode d'exécution actif.
//! - `get_traces` : retourne la liste des traces déjà importées pour le mode actif.
//! - `get_trace_points` : retourne les points d'une trace avec altitude et distance
//!   cumulée (re-parse le GPX original à la demande).
//! - `save_keyframes` / `get_keyframes` / `delete_keyframes` : persistance JSON des
//!   keyframes de la vue d'édition caméra (un fichier par trace).
//!
//! Les données sont stockées dans `{app_data_dir}/{active_mode}/traces/{trace_id}/` et
//! `{app_data_dir}/{active_mode}/traces.json`, conformément au pattern établi
//! par `settings.rs` et `gestionMode.rs`.

use std::io::BufReader;
use std::path::{Path, PathBuf};
use tauri::Manager;

// ---------------------------------------------------------------------------
// Structures de données sérialisables (snake_case → miroir des interfaces TS)
// ---------------------------------------------------------------------------

/// Coordonnées géographiques d'un point (latitude, longitude, altitude).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Point3D {
    pub lat: f64,
    pub lon: f64,
    pub alt: Option<f64>,
}

/// Point de trace enrichi (latitude, longitude, altitude, distance cumulée).
///
/// Miroir exact de l'interface TS `TracePoint` (stores/traces.ts).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TracePoint {
    pub lat: f64,
    pub lon: f64,
    pub alt: Option<f64>,
    /// Distance cumulée 3D depuis le départ (mètres).
    pub distance_m: f64,
}

/// Réponse de la commande `get_trace_points`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TracePoints {
    pub id: String,
    pub points: Vec<TracePoint>,
}

/// Statistiques calculées à partir des points de la trace.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TraceStats {
    pub start_point: Point3D,
    pub end_point: Point3D,
    pub distance_m: f64,
    pub positive_elevation_m: f64,
    pub negative_elevation_m: f64,
    pub alt_min_m: Option<f64>,
    pub alt_max_m: Option<f64>,
    pub points_count: usize,
    pub duration_s: Option<f64>,
}

/// Métadonnées complètes d'une trace importée.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TraceMetadata {
    /// Identifiant unique (UUID v4).
    pub id: String,
    /// Nom de la trace (extrait du GPX ou du nom de fichier).
    pub name: String,
    /// Éditeur / plateforme source (Strava, Garmin Connect, etc.).
    pub source: String,
    /// URL source si détectée.
    pub source_url: Option<String>,
    /// Type d'activité (running, cycling, hiking…).
    pub activity_type: Option<String>,
    /// Nom du fichier GPX stocké dans le dossier de la trace.
    pub filename: String,
    /// Date et heure d'import (ISO 8601 UTC).
    pub import_date: String,
    /// Statistiques calculées.
    pub stats: TraceStats,
    /// Empreinte SHA256 du fichier original, préfixée "sha256:".
    pub hash: String,
    /// Indique si la trace est marquée comme favorite.
    #[serde(default)]
    pub favorite: bool,
    /// Indique si la trace est affichée sur la carte.
    #[serde(default)]
    pub is_displayed: bool,
    /// Statut de nettoyage de la trace : `"clean"` (aucune anomalie détectée ou
    /// déjà nettoyée), `"needs_review"` (anomalies à corriger), `"in_progress"`
    /// (corrections commencées, fichier de travail présent). Absent dans les
    /// registres antérieurs → `"clean"` (rétrocompatibilité).
    #[serde(default = "default_cleaning_status")]
    pub cleaning_status: String,
    /// Phase du pipeline de nettoyage en cours : `"spike"` (pts hors trace),
    /// `"roundabout"` (ronds-points), `"out_and_back"` (aller/retour — étape 3
    /// à venir), ou chaîne vide quand la trace est propre (aucun nettoyage).
    #[serde(default)]
    pub cleaning_phase: String,
    /// Statut d'audit de la trace (module Audit GPX) : `"clean"` (auditée sans
    /// anomalie, ou corrections appliquées), `"needs_review"` (anomalies
    /// détectées à l'import), `"in_progress"`.
    ///
    /// Phase 4 : champ **additif** — `cleaning_status` / `cleaning_phase`
    /// restent maintenus pour l'ancien module jusqu'à la bascule de la Phase 5.
    /// Absent dans les registres antérieurs → `"needs_review"` (une trace
    /// jamais auditée doit l'être).
    #[serde(default = "default_audit_status")]
    pub audit_status: String,
}

/// Valeur par défaut du statut d'audit pour les registres antérieurs :
/// `"needs_review"` — aucune trace n'a été auditée avant la Phase 4.
fn default_audit_status() -> String {
    "needs_review".to_string()
}

/// Valeur par défaut du statut de nettoyage pour les registres antérieurs :
/// `"clean"` (et non la chaîne vide produite par `String::default()`).
fn default_cleaning_status() -> String {
    "clean".to_string()
}

// ---------------------------------------------------------------------------
// Résolution des chemins en fonction du mode d'exécution actif
// ---------------------------------------------------------------------------

/// Retourne le dossier racine du mode d'exécution actif.
/// Exemple : `{app_data_dir}/OPE` ou `{app_data_dir}/EVAL_essai`.
///
/// Point de passage unique de toutes les commandes : déclenche la **migration
/// du stockage** vers « un dossier par trace » au premier accès au mode
/// (idempotente — no-op si `{mode}/traces` existe déjà).
pub(crate) fn get_mode_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let is_dev = cfg!(debug_assertions);
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Dossier app_data introuvable : {}", e))?;
    let active_mode = crate::gestionMode::read_active_mode(&app_data_dir, is_dev);
    let mode_dir = app_data_dir.join(&active_mode);

    // Créer le répertoire du mode s'il n'existe pas (cohérent avec settings.rs)
    if !mode_dir.exists() {
        std::fs::create_dir_all(&mode_dir)
            .map_err(|e| format!("Impossible de créer le dossier du mode {:?} : {}", mode_dir, e))?;
    }

    migrate_mode_storage(&mode_dir)?;

    Ok(mode_dir)
}

/// Retourne le dossier d'une trace : `{mode}/traces/{trace_id}/` (créé si
/// absent). Le dossier est le **discriminant** de la trace : tous les fichiers
/// qui la concernent y vivent (GPX, GeoJSON, keyframes, nettoyage).
pub(crate) fn get_trace_dir(mode_dir: &Path, trace_id: &str) -> Result<PathBuf, String> {
    let dir = mode_dir.join("traces").join(trace_id);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Impossible de créer le dossier de la trace : {}", e))?;
    Ok(dir)
}

/// Chemin du fichier GPX d'une trace (nom d'origine sanitizé dans son dossier).
pub(crate) fn get_trace_gpx_path(mode_dir: &Path, trace_id: &str, filename: &str) -> PathBuf {
    mode_dir.join("traces").join(trace_id).join(filename)
}

/// Retourne le chemin du fichier LineString GeoJSON d'une trace, dans son
/// dossier (`traces/{trace_id}/trace.geojson`).
pub(crate) fn get_geojson_path(mode_dir: &Path, trace_id: &str) -> PathBuf {
    mode_dir.join("traces").join(trace_id).join("trace.geojson")
}

/// Retourne le chemin du registre traces.json du mode actif.
pub(crate) fn get_traces_path(mode_dir: &Path) -> PathBuf {
    mode_dir.join("traces.json")
}

/// Nom de fichier keyframes d'une trace pour un ratio d'écran donné.
///
/// Nommage **explicite par ratio** dans le dossier de la trace :
/// `keyframes_169.json` (16:9) et `keyframes_43.json` (4:3). Tout ratio
/// inconnu retombe sur le nom non suffixé (`keyframes.json`).
fn keyframes_file_name(viewport_aspect: &str) -> String {
    match viewport_aspect {
        "16:9" => "keyframes_169.json".to_string(),
        "4:3" => "keyframes_43.json".to_string(),
        _ => "keyframes.json".to_string(),
    }
}

/// Retourne le chemin du fichier keyframes d'une trace, d'après son UUID et le
/// ratio d'écran (`16:9` ou `4:3`) — un fichier distinct par ratio, dans le
/// dossier de la trace.
fn get_keyframes_path(mode_dir: &Path, trace_id: &str, viewport_aspect: &str) -> PathBuf {
    mode_dir
        .join("traces")
        .join(trace_id)
        .join(keyframes_file_name(viewport_aspect))
}

// ---------------------------------------------------------------------------
// Hash SHA256
// ---------------------------------------------------------------------------

/// Calcule le hash SHA256 du contenu binaire d'un fichier.
/// Retourne une chaîne préfixée "sha256:…".
pub(crate) fn compute_file_hash(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).map_err(|e| format!("Lecture du fichier : {}", e))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let result = hasher.finalize();
    Ok(format!("sha256:{}", hex::encode(result)))
}

// ---------------------------------------------------------------------------
// Utilitaires : nettoyage de nom de fichier
// ---------------------------------------------------------------------------

/// Remplace les espaces et caractères spéciaux par des underscores.
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Migration vers l'agencement « un dossier par trace »
// ---------------------------------------------------------------------------

/// Migre le stockage d'un mode vers `{mode}/traces/{trace_id}/` (un dossier par
/// trace). **Idempotente** : ne fait rien si le dossier `{mode}/traces` existe
/// déjà.
///
/// Pour chaque trace du registre, déplace depuis l'ancien agencement plat
/// (`gpx/`, `geojson/`, `keyframes/`, `cleaning/`) vers son dossier :
/// - `gpx/{filename}` (+ `.gpx.orig`) → `traces/{id}/{filename}` ;
/// - `geojson/{id}.geojson` → `traces/{id}/trace.geojson` ;
/// - `keyframes/{id}_169|_43|.json` → `traces/{id}/keyframes_169|_43|.json` ;
/// - `cleaning/{id}.{phase}*.json` → `traces/{id}/cleaning.{phase}*.json`.
///
/// Les anciens dossiers, une fois vidés, sont supprimés. Les fichiers légués
/// non migrés (ex. `cleaning/{id}.json` sans phase, déjà invalides) restent
/// dans l'ancien dossier et disparaissent avec lui.
pub(crate) fn migrate_mode_storage(mode_dir: &Path) -> Result<(), String> {
    let new_root = mode_dir.join("traces");
    if new_root.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(&new_root)
        .map_err(|e| format!("Création du dossier traces/ : {}", e))?;

    let traces_path = get_traces_path(mode_dir);
    let registry = load_registry(&traces_path);
    if registry.is_empty() {
        return Ok(());
    }

    let move_into = |old: &Path, new: &Path| -> Result<(), String> {
        if old.exists() {
            if let Some(parent) = new.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("Création du dossier : {}", e))?;
            }
            std::fs::rename(old, new)
                .map_err(|e| format!("Déplacement {} → {} : {}", old.display(), new.display(), e))?;
        }
        Ok(())
    };

    for trace in &registry {
        let id = &trace.id;
        let trace_dir = new_root.join(id);
        std::fs::create_dir_all(&trace_dir)
            .map_err(|e| format!("Création du dossier de la trace {} : {}", id, e))?;

        // GPX + backup `.orig`.
        move_into(
            &mode_dir.join("gpx").join(&trace.filename),
            &trace_dir.join(&trace.filename),
        )?;
        move_into(
            &mode_dir.join("gpx").join(format!("{}.gpx.orig", trace.filename)),
            &trace_dir.join(format!("{}.gpx.orig", trace.filename)),
        )?;

        // GeoJSON.
        move_into(
            &mode_dir.join("geojson").join(format!("{id}.geojson")),
            &trace_dir.join("trace.geojson"),
        )?;

        // Keyframes (16:9, 4:3, ancien nom non suffixé).
        for (old_suffix, new_name) in [
            ("_169", "keyframes_169.json"),
            ("_43", "keyframes_43.json"),
            ("", "keyframes.json"),
        ] {
            move_into(
                &mode_dir.join("keyframes").join(format!("{id}{old_suffix}.json")),
                &trace_dir.join(new_name),
            )?;
        }

        // Fichiers de nettoyage par phase (`{id}.{phase}*.json`).
        if let Ok(entries) = std::fs::read_dir(mode_dir.join("cleaning")) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let prefix = format!("{id}.");
                if name.starts_with(&prefix) && name.ends_with(".json") {
                    let rest = &name[id.len() + 1..]; // "spike.json", "spike.decisions.json"
                    move_into(&entry.path(), &trace_dir.join(format!("cleaning.{rest}")))?;
                }
            }
        }
    }

    // Retirer les anciens dossiers s'ils sont vides.
    for dir in ["gpx", "geojson", "keyframes", "cleaning"] {
        let p = mode_dir.join(dir);
        let empty = p
            .read_dir()
            .map(|mut it| it.next().is_none())
            .unwrap_or(false);
        if empty {
            let _ = std::fs::remove_dir(&p);
        }
    }

    println!("[storage] Migration vers « un dossier par trace » effectuée ({} trace(s)).", registry.len());
    Ok(())
}

// ---------------------------------------------------------------------------
// Détection de l'éditeur et URL source
// ---------------------------------------------------------------------------

/// Résultat de la détection heuristique de l'éditeur d'un fichier GPX.
struct EditorDetection {
    name: String,
    source_url: Option<String>,
}

/// Détecte l'éditeur à partir des métadonnées du GPX.
/// Priorité : <link href> → <author><name> → signature XML → "Inconnu".
fn detect_editor(gpx: &gpx::Gpx) -> EditorDetection {
    // 1. Priorité : balise <link href="…"> sur les tracks et dans les metadata
    let mut links: Vec<&gpx::Link> = gpx
        .tracks
        .iter()
        .flat_map(|t| t.links.iter())
        .collect();

    // Ajouter les liens du metadata (si présent)
    if let Some(ref metadata) = gpx.metadata {
        links.extend(metadata.links.iter());
    }

    for link in &links {
        let href = &link.href;
        if let Some(editor) = detect_editor_from_url(href) {
            return EditorDetection {
                name: editor,
                source_url: Some(href.clone()),
            };
        }
    }

    // 2. Fallback : <creator> (champ Gpx.creator)
    if let Some(creator) = &gpx.creator {
        if let Some(editor) = detect_editor_from_name(creator) {
            return EditorDetection {
                name: editor,
                source_url: None,
            };
        }
    }

    // 3. Fallback : nom de la première track comme indicateur
    if let Some(first_track) = gpx.tracks.first() {
        if let Some(track_name) = &first_track.name {
            if let Some(editor) = detect_editor_from_name(track_name) {
                return EditorDetection {
                    name: editor,
                    source_url: None,
                };
            }
        }
    }

    // 4. Fallback final
    EditorDetection {
        name: "Inconnu".to_string(),
        source_url: None,
    }
}

/// Identifie un éditeur connu à partir de l'URL.
fn detect_editor_from_url(url: &str) -> Option<String> {
    let lower = url.to_lowercase();
    if lower.contains("strava.com") {
        return Some("Strava".to_string());
    }
    if lower.contains("connect.garmin.com") {
        return Some("Garmin Connect".to_string());
    }
    if lower.contains("openrunner.com") {
        return Some("OpenRunner".to_string());
    }
    if lower.contains("ridewithgps.com") {
        return Some("RideWithGPS".to_string());
    }

    // Extraire le nom de domaine comme éditeur générique
    if let Some(domain) = url.split("://").nth(1).and_then(|rest| rest.split('/').next()) {
        if !domain.is_empty() {
            return Some(domain.to_string());
        }
    }

    None
}

/// Identifie un éditeur connu à partir d'un nom (creator, author, etc.).
fn detect_editor_from_name(name: &str) -> Option<String> {
    let lower = name.to_lowercase();
    if lower.contains("garmin") {
        return Some("Garmin Connect".to_string());
    }
    if lower.contains("strava") {
        return Some("Strava".to_string());
    }
    if lower.contains("openrunner") {
        return Some("OpenRunner".to_string());
    }
    if lower.contains("ridewithgps") {
        return Some("RideWithGPS".to_string());
    }
    None
}

// ---------------------------------------------------------------------------
// Formule de Haversine
// ---------------------------------------------------------------------------

/// Distance en mètres entre deux points géographiques (formule de Haversine).
pub(crate) fn haversine(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6_371_000.0; // Rayon terrestre moyen en mètres

    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let delta_lat = (lat2 - lat1).to_radians();
    let delta_lon = (lon2 - lon1).to_radians();

    let a = (delta_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    R * c
}

// ---------------------------------------------------------------------------
// Extraction des statistiques depuis un GPX
// ---------------------------------------------------------------------------

/// Nettoie le nom d'une trace en retirant les suffixes d'ID de plateforme.
///
/// OpenRunner ajoute un suffixe `-XXXXXXXX` (ou ` XXXXXXXX`) au nom de la trace.
/// Exemple : "2024 Santa susanna 122 col turo de home-18239499" → "2024 Santa susanna 122 col turo de home"
fn clean_trace_name(name: &str) -> String {
    // Regex : tiret ou espace suivi d'un ID numérique en fin de chaîne
    let re = regex::Regex::new(r"[-\s]\d{5,}\s*$").unwrap();
    let cleaned = re.replace(name, "").trim().to_string();
    if cleaned.is_empty() {
        name.to_string()
    } else {
        cleaned
    }
}

/// Extrait le nom de la trace (priorité : <name> de la première track, puis nom du fichier).
pub(crate) fn extract_name(gpx: &gpx::Gpx, fallback_filename: &str) -> String {
    // Priorité 1 : nom de la première track (nettoyé des ID de plateforme)
    if let Some(track) = gpx.tracks.first() {
        if let Some(name) = &track.name {
            if !name.is_empty() {
                return clean_trace_name(name);
            }
        }
    }

    // Priorité 2 : nom du GPX lui-même (champ metadata.name, nettoyé)
    if let Some(ref metadata) = gpx.metadata {
        if let Some(ref name) = metadata.name {
            if !name.is_empty() {
                return clean_trace_name(name);
            }
        }
    }

    // Fallback : nom du fichier sans extension (nettoyé)
    let raw = Path::new(fallback_filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Trace sans nom");
    clean_trace_name(raw)
}

/// Extrait le type d'activité à partir du premier segment qui le définit.
fn extract_activity_type(gpx: &gpx::Gpx) -> Option<String> {
    for track in &gpx.tracks {
        if let Some(ref activity) = track.type_ {
            if !activity.is_empty() {
                return Some(activity.clone());
            }
        }
    }
    None
}

/// Calcul complet des statistiques d'un GPX.
/// Retourne un tuple (stats, nombre_total_de_points).
pub(crate) fn compute_stats(gpx: &gpx::Gpx) -> Result<(TraceStats, usize), String> {
    // Collecter tous les points dans l'ordre (coords, altitude, timestamp)
    let mut all_points: Vec<(f64, f64, Option<f64>, Option<time::OffsetDateTime>)> =
        Vec::new();

    for track in &gpx.tracks {
        for segment in &track.segments {
            for wp in &segment.points {
                let pt = wp.point();
                let lat = pt.y();
                let lon = pt.x();
                // Convertir gpx::Time en time::OffsetDateTime
                let time_opt = wp.time.map(|t| time::OffsetDateTime::from(t));
                all_points.push((lat, lon, wp.elevation, time_opt));
            }
        }
    }

    if all_points.is_empty() {
        return Err("Le fichier GPX ne contient aucun point.".to_string());
    }

    let points_count = all_points.len();

    // Point de départ et d'arrivée
    let (start_lat, start_lon, start_alt, _) = all_points[0];
    let (end_lat, end_lon, end_alt, _) = all_points[points_count - 1];

    let start_point = Point3D {
        lat: start_lat,
        lon: start_lon,
        alt: start_alt,
    };

    let end_point = Point3D {
        lat: end_lat,
        lon: end_lon,
        alt: end_alt,
    };

    // Calcul de la distance cumulée et des dénivelés
    let mut total_distance = 0.0_f64;
    let mut positive_elevation = 0.0_f64;
    let mut negative_elevation = 0.0_f64;
    let mut alt_min = f64::MAX;
    let mut alt_max = f64::MIN;
    let mut has_altitude = false;

    for window in all_points.windows(2) {
        let (lat1, lon1, alt1_opt, _) = window[0];
        let (lat2, lon2, alt2_opt, _) = window[1];

        // Distance 3D
        let dist_2d = haversine(lat1, lon1, lat2, lon2);
        let delta_alt = match (alt1_opt, alt2_opt) {
            (Some(a1), Some(a2)) => {
                let da = a2 - a1;
                // Dénivelés
                if da > 0.0 {
                    positive_elevation += da;
                } else {
                    negative_elevation += da.abs();
                }
                da
            }
            _ => 0.0, // altitude manquante → delta_alt = 0
        };
        let dist_3d = (dist_2d.powi(2) + delta_alt.powi(2)).sqrt();
        total_distance += dist_3d;
    }

    // Altitudes min / max
    for (_, _, alt_opt, _) in &all_points {
        if let Some(alt) = alt_opt {
            has_altitude = true;
            if *alt < alt_min {
                alt_min = *alt;
            }
            if *alt > alt_max {
                alt_max = *alt;
            }
        }
    }

    // Durée estimée (différence entre le premier et le dernier timestamp)
    let first_time = all_points.first().and_then(|p| p.3);
    let last_time = all_points.last().and_then(|p| p.3);
    let duration_s = match (first_time, last_time) {
        (Some(t_first), Some(t_last)) => {
            let delta = t_last - t_first;
            Some(delta.whole_seconds() as f64)
        }
        _ => None,
    };

    let stats = TraceStats {
        start_point: start_point.clone(),
        end_point: end_point.clone(),
        distance_m: total_distance,
        positive_elevation_m: positive_elevation,
        negative_elevation_m: negative_elevation,
        alt_min_m: if has_altitude { Some(alt_min) } else { None },
        alt_max_m: if has_altitude { Some(alt_max) } else { None },
        points_count,
        duration_s,
    };

    Ok((stats, points_count))
}

// ---------------------------------------------------------------------------
// Extraction des coordonnées LineString (au format Mapbox [lon, lat])
// ---------------------------------------------------------------------------

/// Extrait les coordonnées de tous les points du GPX au format Mapbox `[lon, lat]`.
///
/// Réutilise la même boucle d'itération que `compute_stats` (`tracks → segments → points`).
/// Le GPX fournit `lat = pt.y()` et `lon = pt.x()`, d'où `[lon, lat] = [pt.x(), pt.y()]`.
/// Lève une erreur explicite si le GPX contient strictement moins de 2 points
/// (une LineString valide en nécessite au moins 2).
pub(crate) fn extract_line_coordinates(gpx: &gpx::Gpx) -> Result<Vec<[f64; 2]>, String> {
    let mut coords: Vec<[f64; 2]> = Vec::new();

    for track in &gpx.tracks {
        for segment in &track.segments {
            for wp in &segment.points {
                let pt = wp.point();
                // Coordonnées Mapbox : [longitude, latitude] = [pt.x(), pt.y()]
                coords.push([pt.x(), pt.y()]);
            }
        }
    }

    if coords.len() < 2 {
        return Err(
            "Le fichier GPX contient moins de 2 points : impossible de créer une LineString."
                .to_string(),
        );
    }

    Ok(coords)
}

/// Construit une Feature GeoJSON (LineString) à partir des coordonnées et de la trace.
///
/// Le `properties.id` reprend l'UUID de la trace (clé de liaison avec `TraceMetadata`).
pub(crate) fn build_geojson_feature(coords: Vec<[f64; 2]>, id: &str, name: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "Feature",
        "geometry": {
            "type": "LineString",
            "coordinates": coords,
        },
        "properties": {
            "id": id,
            "name": name,
        }
    })
}

// ---------------------------------------------------------------------------
// Extraction des points avec distance cumulée (pour la vue d'édition caméra)
// ---------------------------------------------------------------------------

/// Extrait tous les points d'un GPX avec altitude et distance cumulée 3D.
///
/// Réutilise le même pattern d'itération et la formule Haversine que
/// `compute_stats`. Le premier point a `distance_m = 0`. La distance
/// cumulée est calculée en 3D (Haversine + delta_alt).
fn extract_points_with_distance(gpx: &gpx::Gpx) -> Result<Vec<TracePoint>, String> {
    let mut points: Vec<TracePoint> = Vec::new();
    let mut cumulated_distance = 0.0_f64;
    let mut prev: Option<(f64, f64, Option<f64>)> = None; // (lat, lon, alt)

    for track in &gpx.tracks {
        for segment in &track.segments {
            for wp in &segment.points {
                let pt = wp.point();
                let lat = pt.y();
                let lon = pt.x();
                let alt = wp.elevation;

                // Calculer la distance 3D par rapport au point précédent.
                if let Some((prev_lat, prev_lon, prev_alt)) = prev {
                    let dist_2d = haversine(prev_lat, prev_lon, lat, lon);
                    let delta_alt = match (prev_alt, alt) {
                        (Some(a1), Some(a2)) => a2 - a1,
                        _ => 0.0,
                    };
                    let dist_3d = (dist_2d.powi(2) + delta_alt.powi(2)).sqrt();
                    cumulated_distance += dist_3d;
                }

                points.push(TracePoint {
                    lat,
                    lon,
                    alt,
                    distance_m: cumulated_distance,
                });

                prev = Some((lat, lon, alt));
            }
        }
    }

    if points.is_empty() {
        return Err("Le fichier GPX ne contient aucun point.".to_string());
    }

    Ok(points)
}

// ---------------------------------------------------------------------------
// Gestion du registre traces.json
// ---------------------------------------------------------------------------

/// Charge le registre des traces depuis le fichier JSON.
/// Retourne un vecteur vide si le fichier n'existe pas ou est illisible.
pub(crate) fn load_registry(traces_path: &Path) -> Vec<TraceMetadata> {
    if !traces_path.exists() {
        return Vec::new();
    }

    match std::fs::read_to_string(traces_path) {
        Ok(content) => match serde_json::from_str::<Vec<TraceMetadata>>(&content) {
            Ok(mut registry) => {
                // Rétrocompatibilité : normalise le statut de nettoyage et la
                // phase des registres antérieurs ou corrompus.
                //  - statut vide → "clean" ;
                //  - trace « clean » → phase vide (aucun nettoyage requis) ;
                //  - trace à nettoyer (needs_review / in_progress) sans phase
                //    persistée → première étape (re-détection en chaîne).
                for trace in &mut registry {
                    if trace.cleaning_status.is_empty() {
                        trace.cleaning_status = "clean".to_string();
                    }
                    if trace.cleaning_phase.is_empty() {
                        trace.cleaning_phase = if trace.cleaning_status == "clean" {
                            String::new()
                        } else {
                            "spike".to_string()
                        };
                    }
                }
                registry
            }
            Err(e) => {
                eprintln!("Erreur de lecture du registre traces.json : {}", e);
                Vec::new()
            }
        },
        Err(e) => {
            eprintln!("Impossible de lire traces.json : {}", e);
            Vec::new()
        }
    }
}

/// Sauvegarde le registre des traces avec une écriture atomique (fichier tmp + rename).
pub(crate) fn save_registry(traces_path: &Path, registry: &[TraceMetadata]) -> Result<(), String> {
    let tmp_path = traces_path.with_extension("json.tmp");
    let content =
        serde_json::to_string_pretty(registry).map_err(|e| format!("Sérialisation JSON : {}", e))?;
    std::fs::write(&tmp_path, content)
        .map_err(|e| format!("Écriture du registre temporaire : {}", e))?;
    std::fs::rename(&tmp_path, traces_path)
        .map_err(|e| format!("Renommage du registre : {}", e))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Commande Tauri : import_gpx_file
// ---------------------------------------------------------------------------

/// Importe un fichier GPX sélectionné par l'utilisateur via le sélecteur natif.
///
/// Le fichier est copié dans `{app_data_dir}/{active_mode}/gpx/` et les
/// métadonnées sont enregistrées dans `{app_data_dir}/{active_mode}/traces.json`.
#[tauri::command]
pub async fn import_gpx_file(app: tauri::AppHandle) -> Result<TraceMetadata, String> {
    // macOS : contournement d'un bug AppKit/ViewBridge connu — `+[NSOpenPanel
    // openPanel]` peut retourner NULL (voire rester bloqué avec la roue
    // colorée) quand un panneau précédent n'a pas fini d'être démonté par le
    // service système. Un court délai avant d'ouvrir le sélecteur laisse le
    // temps au service de se stabiliser (surtout pour les ouvertures
    // successives). Sans effet notable dans les autres cas.
    #[cfg(target_os = "macos")]
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let start_time = std::time::Instant::now();

    // 1. Ouvrir le sélecteur de fichier natif (fichier unique, filtre .gpx)
    use tauri_plugin_dialog::DialogExt;

    let file_path = app
        .dialog()
        .file()
        .add_filter("Fichier GPX", &["gpx"])
        .set_title("Importer une trace GPX")
        .blocking_pick_file();

    let file_path = match file_path {
        Some(path) => path,
        None => return Err("Aucun fichier sélectionné.".to_string()),
    };
    let file_path: PathBuf = file_path.into_path().map_err(|e| e.to_string())?;

    // Sécurité : vérifier l'extension .gpx
    let extension = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if !extension.eq_ignore_ascii_case("gpx") {
        return Err("Le fichier sélectionné n'est pas un fichier .gpx.".to_string());
    }

    // 2. Résoudre les chemins du mode d'exécution actif
    let mode_dir = get_mode_dir(&app)?;
    let traces_path = get_traces_path(&mode_dir);

    // 3. Calculer le hash SHA256 (anti-doublon)
    let hash = compute_file_hash(&file_path)?;

    // 4. Charger le registre et vérifier les doublons
    let mut registry = load_registry(&traces_path);
    if registry.iter().any(|t| t.hash == hash) {
        return Err("Cette trace a déjà été importée.".to_string());
    }

    // 5. Parser le fichier GPX
    let file = std::fs::File::open(&file_path)
        .map_err(|e| format!("Ouverture du fichier : {}", e))?;
    let reader = BufReader::new(file);
    let gpx = gpx::read(reader).map_err(|e| format!("Fichier GPX invalide : {}", e))?;

    // 6. Extraire les métadonnées et calculer les statistiques
    let name = extract_name(&gpx, &file_path.to_string_lossy());
    let activity_type = extract_activity_type(&gpx);
    let detection = detect_editor(&gpx);
    let (stats, _) = compute_stats(&gpx)?;

    // 6bis. Détecter les anomalies des **3 étapes** du pipeline de nettoyage
    //       (points hors trace, ronds-points, aller-retours) : une trace est
    //       signalée « à nettoyer » dès qu'une anomalie existe, quelle que soit
    //       son étape. Tolérance de cap `Nettoyage.Cap.toleranceDeg` et
    //       paramètres `Nettoyage.RondPoints.*` lus depuis les réglages.
    //
    //       La détection est **protégée contre tout panic imprévu** : un panic
    //       dans une commande async laisserait l'appelant (le frontend) bloqué
    //       sans réponse — l'import resterait muet après la sélection du
    //       fichier. En cas de défaillance, on replie sur « aucune anomalie ».
    let tolerance = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::cleaning::read_tolerance_deg(&app)
    }))
    .unwrap_or(5.0);
    let roundabout_params = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::cleaning::read_roundabout_params(&app)
    }))
    .unwrap_or_default();
    let (spikes, roundabouts, out_and_backs) =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::cleaning::detect_all_phases_from_gpx(&gpx, tolerance, &roundabout_params)
        }))
        .unwrap_or_default();
    let total_anomalies = spikes.len() + roundabouts.len() + out_and_backs.len();
    let cleaning_status = if total_anomalies == 0 {
        "clean".to_string()
    } else {
        "needs_review".to_string()
    };
    // Phase initiale : étape 1 si anomalies, sinon aucune (trace propre).
    let cleaning_phase = if total_anomalies == 0 {
        String::new()
    } else {
        "spike".to_string()
    };
    if total_anomalies > 0 {
        println!(
            "[import_gpx] Trace « {} » : {} anomalie(s) détectée(s) ({} pts hors trace, {} rond(s)-point(s), {} aller-retour(s)), statut « needs_review ».",
            name, total_anomalies, spikes.len(), roundabouts.len(), out_and_backs.len()
        );
    }

    // 6ter. Audit GPX (module `gpx_audit`) : détection AR + RP sur les mêmes
    //       points, avec les paramètres `Audit.*`. La trace est « à auditer »
    //       dès qu'une anomalie est détectée. Protégé contre tout panic, comme
    //       la détection de nettoyage ci-dessus (un panic laisserait l'import
    //       sans réponse).
    //
    //       Phase 4 : les deux statuts coexistent (cf. `TraceMetadata`) ; la
    //       détection d'audit est la seule à piloter le gate de la Phase 5.
    let audit_params = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::gpx_audit::commands::read_audit_params(&app)
    }))
    .unwrap_or_else(|_| crate::gpx_audit::commands::default_audit_params());
    let audit_status = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let raw = crate::gpx_audit::pipeline::raw_points_from_gpx(&gpx);
        crate::gpx_audit::pipeline::detect_all(raw, &audit_params)
    }))
    .map(|outcome| match outcome {
        // Trace trop courte après consolidation : aucune anomalie AR/RP
        // possible, la trace est réputée propre.
        Ok(outcome) if !outcome.findings.is_empty() => "needs_review".to_string(),
        Ok(_) | Err(_) => "clean".to_string(),
    })
    .unwrap_or_else(|_| "needs_review".to_string());

    // 7. Générer l'UUID de la trace et créer son dossier
    //    (`traces/{id}/` — le dossier est le discriminant de la trace).
    let id = uuid::Uuid::new_v4().to_string();
    let trace_dir = get_trace_dir(&mode_dir, &id)?;

    // 7bis. Copier le GPX dans le dossier de la trace (nom d'origine sanitizé ;
    //       pas de collision possible : le dossier est unique par trace).
    let raw_filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("trace.gpx");
    let stored_filename = sanitize_filename(raw_filename);
    let dest_path = trace_dir.join(&stored_filename);

    std::fs::copy(&file_path, &dest_path)
        .map_err(|e| format!("Copie du fichier GPX : {}", e))?;

    // 8. Construire la métadonnée
    let import_date = chrono::Utc::now().to_rfc3339();

    // 8bis. Générer la LineString GeoJSON et l'écrire dans `traces/{id}/trace.geojson`
    //       (après génération de l'UUID, avant la construction de TraceMetadata).
    let coords = extract_line_coordinates(&gpx)?;
    let feature = build_geojson_feature(coords, &id, &name);
    let geojson_path = get_geojson_path(&mode_dir, &id);
    // Écriture atomique (tmp + rename), cohérent avec save_registry.
    let geojson_tmp = geojson_path.with_extension("geojson.tmp");
    let geojson_content = serde_json::to_string_pretty(&feature)
        .map_err(|e| format!("Sérialisation GeoJSON : {}", e))?;
    std::fs::write(&geojson_tmp, geojson_content)
        .map_err(|e| format!("Écriture du fichier GeoJSON temporaire : {}", e))?;
    std::fs::rename(&geojson_tmp, &geojson_path)
        .map_err(|e| format!("Renommage du fichier GeoJSON : {}", e))?;

    let metadata = TraceMetadata {
        id,
        name,
        source: detection.name,
        source_url: detection.source_url,
        activity_type,
        filename: stored_filename,
        import_date,
        stats,
        hash,
        favorite: false,
        is_displayed: false,
        cleaning_status,
        cleaning_phase,
        audit_status,
    };

    // 9. Ajouter au registre et sauvegarder (écriture atomique)
    registry.push(metadata.clone());
    save_registry(&traces_path, &registry)?;

    let elapsed = start_time.elapsed();
    println!(
        "[import_gpx] Trace « {} » importée en {} ms ({} points, {:.1} km).",
        metadata.name,
        elapsed.as_millis(),
        metadata.stats.points_count,
        metadata.stats.distance_m / 1000.0
    );

    Ok(metadata)
}

// ---------------------------------------------------------------------------
// Commande Tauri : get_traces
// ---------------------------------------------------------------------------

/// Retourne la liste des traces importées pour le mode d'exécution actif.
#[tauri::command]
pub async fn get_traces(app: tauri::AppHandle) -> Result<Vec<TraceMetadata>, String> {
    let mode_dir = get_mode_dir(&app)?;
    let traces_path = get_traces_path(&mode_dir);
    Ok(load_registry(&traces_path))
}

// ---------------------------------------------------------------------------
// Commande Tauri : delete_trace
// ---------------------------------------------------------------------------

/// Supprime une trace (dossier entier + entrée du registre) par son identifiant.
///
/// Tous les fichiers d'une trace vivent dans son dossier `traces/{id}/` (GPX,
/// backup `.orig`, GeoJSON, keyframes, nettoyage) : la suppression du dossier
/// **est** la cascade complète.
#[tauri::command]
pub async fn delete_trace(app: tauri::AppHandle, trace_id: String) -> Result<(), String> {
    let mode_dir = get_mode_dir(&app)?;
    let traces_path = get_traces_path(&mode_dir);

    let mut registry = load_registry(&traces_path);

    // Recherche par id (clé unique UUID v4, pas par filename)
    let idx = registry
        .iter()
        .position(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable (id={})", trace_id))?;

    let trace_id_owned = registry[idx].id.clone();

    // 1) Supprimer le dossier de la trace (tolérant si absent) — contient le
    //    GPX, le backup `.orig`, le GeoJSON, les keyframes et les fichiers de
    //    nettoyage.
    let trace_dir = mode_dir.join("traces").join(&trace_id_owned);
    if trace_dir.exists() {
        std::fs::remove_dir_all(&trace_dir)
            .map_err(|e| format!("Suppression du dossier de la trace : {}", e))?;
    }

    // 2) Retirer l'entrée du registre en mémoire
    registry.remove(idx);

    // 3) Sauvegarde atomique (tmp + rename via save_registry)
    save_registry(&traces_path, &registry)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Commande Tauri : update_trace
// ---------------------------------------------------------------------------

/// Met à jour partiellement les métadonnées d'une trace (favori, affichage).
///
/// Seuls les champs fournis (non `None`) sont modifiés. Le registre est
/// sauvegardé de manière atomique après la mutation.
#[tauri::command]
pub async fn update_trace(
    app: tauri::AppHandle,
    trace_id: String,
    favorite: Option<bool>,
    is_displayed: Option<bool>,
) -> Result<(), String> {
    let mode_dir = get_mode_dir(&app)?;
    let traces_path = get_traces_path(&mode_dir);

    let mut registry = load_registry(&traces_path);

    let trace = registry
        .iter_mut()
        .find(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable (id={})", trace_id))?;

    if let Some(fav) = favorite {
        trace.favorite = fav;
    }
    if let Some(disp) = is_displayed {
        trace.is_displayed = disp;
    }

    save_registry(&traces_path, &registry)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Commande Tauri : get_trace_geometry
// ---------------------------------------------------------------------------

/// Géométrie (LineString GeoJSON) d'une trace, retournée par `get_trace_geometry`.
#[derive(serde::Serialize)]
pub struct TraceGeometry {
    /// Identifiant (UUID) de la trace.
    pub id: String,
    /// Feature GeoJSON (LineString) de la trace.
    pub geometry: serde_json::Value,
}

/// Régénère le fichier `geojson/{trace_id}.geojson` depuis le GPX original.
///
/// Utilisé pour migrer les traces importées avant l'existence du dossier
/// `geojson/` (le fichier LineString n'était pas créé à l'import à l'époque).
/// Retourne la Feature GeoJSON fraîchement écrite. Écriture atomique.
fn rebuild_geometry_from_gpx(
    mode_dir: &Path,
    registry: &[TraceMetadata],
    trace_id: &str,
) -> Result<serde_json::Value, String> {
    let trace = registry
        .iter()
        .find(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable (id={})", trace_id))?;

    let gpx_file = get_trace_gpx_path(mode_dir, trace_id, &trace.filename);
    if !gpx_file.exists() {
        return Err(format!(
            "Fichier GPX source introuvable pour la trace (id={}, fichier={:?})",
            trace_id, gpx_file
        ));
    }

    let file = std::fs::File::open(&gpx_file)
        .map_err(|e| format!("Ouverture du fichier GPX : {}", e))?;
    let reader = BufReader::new(file);
    let gpx = gpx::read(reader).map_err(|e| format!("Fichier GPX invalide : {}", e))?;

    let coords = extract_line_coordinates(&gpx)?;
    let feature = build_geojson_feature(coords, trace_id, &trace.name);

    // Écriture atomique du fichier `traces/{trace_id}/trace.geojson`.
    let geojson_path = get_geojson_path(mode_dir, trace_id);
    let geojson_tmp = geojson_path.with_extension("geojson.tmp");
    let geojson_content = serde_json::to_string_pretty(&feature)
        .map_err(|e| format!("Sérialisation GeoJSON : {}", e))?;
    std::fs::write(&geojson_tmp, geojson_content)
        .map_err(|e| format!("Écriture du fichier GeoJSON temporaire : {}", e))?;
    std::fs::rename(&geojson_tmp, &geojson_path)
        .map_err(|e| format!("Renommage du fichier GeoJSON : {}", e))?;

    Ok(feature)
}

/// Retourne la Feature LineString GeoJSON d'une trace par son identifiant.
///
/// Lit le fichier `{mode_dir}/traces/{trace_id}/trace.geojson` généré à
/// l'import. Si ce fichier manque, il est régénéré automatiquement depuis le
/// GPX original, puis mis en cache.
/// Le `properties.id` de la Feature correspond à l'UUID de la trace.
#[tauri::command]
pub async fn get_trace_geometry(
    app: tauri::AppHandle,
    trace_id: String,
) -> Result<TraceGeometry, String> {
    let mode_dir = get_mode_dir(&app)?;
    let geojson_path = get_geojson_path(&mode_dir, &trace_id);

    // Cas normal : le fichier GeoJSON existe déjà, on le lit.
    if geojson_path.exists() {
        let content = std::fs::read_to_string(&geojson_path)
            .map_err(|e| format!("Lecture du fichier GeoJSON : {}", e))?;
        let geometry: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("Fichier GeoJSON invalide : {}", e))?;
        return Ok(TraceGeometry {
            id: trace_id,
            geometry,
        });
    }

    // Cas de migration : le fichier manque, on le régénère depuis le GPX.
    let traces_path = get_traces_path(&mode_dir);
    let registry = load_registry(&traces_path);
    let geometry = rebuild_geometry_from_gpx(&mode_dir, &registry, &trace_id)?;

    Ok(TraceGeometry {
        id: trace_id,
        geometry,
    })
}

// ---------------------------------------------------------------------------
// Commande Tauri : get_trace_points
// ---------------------------------------------------------------------------

/// Retourne les points d'une trace avec altitude et distance cumulée 3D.
///
/// Re-parse le fichier GPX original à la demande (pas de persistance
/// intermédiaire). La distance cumulée est calculée via Haversine + delta_alt,
/// cohérente avec `compute_stats`.
#[tauri::command]
pub async fn get_trace_points(
    app: tauri::AppHandle,
    trace_id: String,
) -> Result<TracePoints, String> {
    let mode_dir = get_mode_dir(&app)?;
    let traces_path = get_traces_path(&mode_dir);
    let registry = load_registry(&traces_path);

    // Trouver le nom de fichier GPX de la trace.
    let trace = registry
        .iter()
        .find(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable (id={})", trace_id))?;

    // Re-parser le GPX original (dans le dossier de la trace).
    let gpx_file = get_trace_gpx_path(&mode_dir, &trace_id, &trace.filename);
    if !gpx_file.exists() {
        return Err(format!(
            "Fichier GPX source introuvable pour la trace (id={}, fichier={:?})",
            trace_id, gpx_file
        ));
    }

    let file = std::fs::File::open(&gpx_file)
        .map_err(|e| format!("Ouverture du fichier GPX : {}", e))?;
    let reader = BufReader::new(file);
    let gpx = gpx::read(reader).map_err(|e| format!("Fichier GPX invalide : {}", e))?;

    let points = extract_points_with_distance(&gpx)?;

    Ok(TracePoints {
        id: trace_id,
        points,
    })
}

// ---------------------------------------------------------------------------
// Commandes Tauri : persistance des keyframes
// ---------------------------------------------------------------------------

/// Sauvegarde un jeu de keyframes (sérialisé en JSON par le frontend).
///
/// Le JSON est écrit dans `{mode_dir}/traces/{trace_id}/keyframes_{ratio}.json`
/// avec une écriture atomique (fichier tmp + rename), cohérente avec
/// `save_registry`. Le dossier de la trace est créé si nécessaire.
#[tauri::command]
pub async fn save_keyframes(
    app: tauri::AppHandle,
    trace_id: String,
    viewport_aspect: String,
    keyframes_json: serde_json::Value,
) -> Result<(), String> {
    let mode_dir = get_mode_dir(&app)?;
    get_trace_dir(&mode_dir, &trace_id)?; // crée le dossier de la trace
    let path = get_keyframes_path(&mode_dir, &trace_id, &viewport_aspect);

    let tmp_path = path.with_extension("json.tmp");
    let content = serde_json::to_string_pretty(&keyframes_json)
        .map_err(|e| format!("Sérialisation keyframes : {}", e))?;
    std::fs::write(&tmp_path, content)
        .map_err(|e| format!("Écriture du fichier keyframes temporaire : {}", e))?;
    std::fs::rename(&tmp_path, &path)
        .map_err(|e| format!("Renommage du fichier keyframes : {}", e))?;

    Ok(())
}

/// Charge les keyframes persistés d'une trace pour un ratio d'écran donné.
///
/// Retourne `Some(serde_json::Value)` si le fichier existe, `None` sinon.
/// Le frontend est responsable du typage (cast vers `KeyframeSet`).
#[tauri::command]
pub async fn get_keyframes(
    app: tauri::AppHandle,
    trace_id: String,
    viewport_aspect: String,
) -> Result<Option<serde_json::Value>, String> {
    let mode_dir = get_mode_dir(&app)?;
    let path = get_keyframes_path(&mode_dir, &trace_id, &viewport_aspect);

    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Lecture du fichier keyframes : {}", e))?;
    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Fichier keyframes invalide : {}", e))?;

    Ok(Some(value))
}

/// Supprime le fichier keyframes d'une trace pour un ratio donné (tolérant si
/// absent).
#[tauri::command]
pub async fn delete_keyframes(
    app: tauri::AppHandle,
    trace_id: String,
    viewport_aspect: String,
) -> Result<(), String> {
    let mode_dir = get_mode_dir(&app)?;
    let path = get_keyframes_path(&mode_dir, &trace_id, &viewport_aspect);

    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| format!("Suppression du fichier keyframes : {}", e))?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Dossier temporaire isolé pour un test (nettoyé au préalable).
    fn test_mode_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vg2_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn make_trace(id: &str, filename: &str) -> TraceMetadata {
        TraceMetadata {
            id: id.to_string(),
            name: "Test trace".to_string(),
            source: "gpx".to_string(),
            source_url: None,
            activity_type: None,
            filename: filename.to_string(),
            import_date: "2026-01-01T00:00:00Z".to_string(),
            stats: TraceStats {
                start_point: Point3D { lat: 41.6, lon: 2.5, alt: None },
                end_point: Point3D { lat: 41.7, lon: 2.6, alt: None },
                distance_m: 1000.0,
                positive_elevation_m: 0.0,
                negative_elevation_m: 0.0,
                alt_min_m: None,
                alt_max_m: None,
                points_count: 2,
                duration_s: None,
            },
            hash: "sha256:test".to_string(),
            favorite: false,
            is_displayed: false,
            cleaning_status: "needs_review".to_string(),
            cleaning_phase: "spike".to_string(),
            audit_status: "needs_review".to_string(),
        }
    }

    /// La migration déplace chaque fichier de l'ancien agencement plat vers le
    /// dossier de la trace, puis retire les anciens dossiers vides.
    #[test]
    fn migrate_mode_storage_moves_files_to_trace_dir() {
        let mode = test_mode_dir("migrate");
        let id = "abc-123";
        let gpx_name = "2024_ma_route.gpx";

        // Ancien agencement.
        fs::create_dir_all(mode.join("gpx")).unwrap();
        fs::create_dir_all(mode.join("geojson")).unwrap();
        fs::create_dir_all(mode.join("keyframes")).unwrap();
        fs::create_dir_all(mode.join("cleaning")).unwrap();
        fs::write(mode.join("gpx").join(gpx_name), "gpx").unwrap();
        fs::write(mode.join("gpx").join(format!("{}.gpx.orig", gpx_name)), "orig").unwrap();
        fs::write(mode.join("geojson").join(format!("{}.geojson", id)), "geo").unwrap();
        fs::write(mode.join("keyframes").join(format!("{}_169.json", id)), "k169").unwrap();
        fs::write(mode.join("keyframes").join(format!("{}_43.json", id)), "k43").unwrap();
        fs::write(mode.join("cleaning").join(format!("{}.spike.json", id)), "w1").unwrap();
        fs::write(mode.join("cleaning").join(format!("{}.spike.decisions.json", id)), "d1").unwrap();

        // Registre.
        let registry = vec![make_trace(id, gpx_name)];
        let traces_path = mode.join("traces.json");
        fs::write(&traces_path, serde_json::to_string(&registry).unwrap()).unwrap();

        migrate_mode_storage(&mode).unwrap();

        let td = mode.join("traces").join(id);
        assert_eq!(fs::read_to_string(td.join(gpx_name)).unwrap(), "gpx");
        assert_eq!(fs::read_to_string(td.join(format!("{}.gpx.orig", gpx_name))).unwrap(), "orig");
        assert_eq!(fs::read_to_string(td.join("trace.geojson")).unwrap(), "geo");
        assert_eq!(fs::read_to_string(td.join("keyframes_169.json")).unwrap(), "k169");
        assert_eq!(fs::read_to_string(td.join("keyframes_43.json")).unwrap(), "k43");
        assert_eq!(fs::read_to_string(td.join("cleaning.spike.json")).unwrap(), "w1");
        assert_eq!(fs::read_to_string(td.join("cleaning.spike.decisions.json")).unwrap(), "d1");
        // Anciens dossiers retirés (vides).
        for d in ["gpx", "geojson", "keyframes", "cleaning"] {
            assert!(!mode.join(d).exists(), "{} devrait avoir été retiré", d);
        }
        // traces.json reste au niveau du mode.
        assert!(traces_path.exists());
    }

    /// La migration est **idempotente** : un second appel ne fait rien (et ne
    /// casse pas les fichiers déjà en place).
    #[test]
    fn migrate_mode_storage_is_idempotent() {
        let mode = test_mode_dir("migrate_idem");
        let id = "def-456";
        let gpx_name = "trace.gpx";

        fs::create_dir_all(mode.join("gpx")).unwrap();
        fs::create_dir_all(mode.join("geojson")).unwrap();
        fs::write(mode.join("gpx").join(gpx_name), "gpx").unwrap();
        fs::write(mode.join("geojson").join(format!("{}.geojson", id)), "geo").unwrap();
        let registry = vec![make_trace(id, gpx_name)];
        fs::write(mode.join("traces.json"), serde_json::to_string(&registry).unwrap()).unwrap();

        migrate_mode_storage(&mode).unwrap();
        migrate_mode_storage(&mode).unwrap();

        let td = mode.join("traces").join(id);
        assert_eq!(fs::read_to_string(td.join(gpx_name)).unwrap(), "gpx");
        assert_eq!(fs::read_to_string(td.join("trace.geojson")).unwrap(), "geo");
    }
}
