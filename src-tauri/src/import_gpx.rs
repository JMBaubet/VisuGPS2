//! Module d'import de fichiers GPX pour VisuGPS2.
//!
//! Ce module expose deux commandes Tauri :
//! - `import_gpx_file` : ouvre un sélecteur natif, parse le fichier, calcule les stats,
//!   stocke le fichier et les métadonnées dans le dossier du mode d'exécution actif.
//! - `get_traces` : retourne la liste des traces déjà importées pour le mode actif.
//!
//! Les données sont stockées dans `{app_data_dir}/{active_mode}/gpx/` et
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
    /// Nom du fichier stocké dans le dossier gpx/.
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
}

// ---------------------------------------------------------------------------
// Résolution des chemins en fonction du mode d'exécution actif
// ---------------------------------------------------------------------------

/// Retourne le dossier racine du mode d'exécution actif.
/// Exemple : `{app_data_dir}/OPE` ou `{app_data_dir}/EVAL_essai`.
fn get_mode_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
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

    Ok(mode_dir)
}

/// Retourne le dossier gpx/ à l'intérieur du mode actif.
fn get_gpx_dir(mode_dir: &Path) -> Result<PathBuf, String> {
    let gpx_dir = mode_dir.join("gpx");
    std::fs::create_dir_all(&gpx_dir)
        .map_err(|e| format!("Impossible de créer le dossier gpx : {}", e))?;
    Ok(gpx_dir)
}

/// Retourne le dossier geojson/ à l'intérieur du mode actif.
/// Contient les LineString GeoJSON des traces (un fichier `{id}.geojson` par trace).
fn get_geojson_dir(mode_dir: &Path) -> Result<PathBuf, String> {
    let geojson_dir = mode_dir.join("geojson");
    std::fs::create_dir_all(&geojson_dir)
        .map_err(|e| format!("Impossible de créer le dossier geojson : {}", e))?;
    Ok(geojson_dir)
}

/// Retourne le chemin du fichier LineString GeoJSON d'une trace, d'après son UUID.
fn get_geojson_path(mode_dir: &Path, trace_id: &str) -> PathBuf {
    mode_dir.join("geojson").join(format!("{}.geojson", trace_id))
}

/// Retourne le chemin du registre traces.json du mode actif.
fn get_traces_path(mode_dir: &Path) -> PathBuf {
    mode_dir.join("traces.json")
}

// ---------------------------------------------------------------------------
// Hash SHA256
// ---------------------------------------------------------------------------

/// Calcule le hash SHA256 du contenu binaire d'un fichier.
/// Retourne une chaîne préfixée "sha256:…".
fn compute_file_hash(path: &Path) -> Result<String, String> {
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

/// Génère un nom de fichier unique dans le dossier cible en cas de conflit.
/// Ex : "trace.gpx" → "trace_1.gpx" si "trace.gpx" existe déjà.
fn unique_filename(gpx_dir: &Path, base_name: &str) -> String {
    if !gpx_dir.join(base_name).exists() {
        return base_name.to_string();
    }

    let stem = Path::new(base_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("trace");
    let ext = Path::new(base_name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("gpx");

    let mut counter = 1u32;
    loop {
        let candidate = format!("{}_{}.{}", stem, counter, ext);
        if !gpx_dir.join(&candidate).exists() {
            return candidate;
        }
        counter += 1;
    }
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
fn haversine(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
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
fn extract_name(gpx: &gpx::Gpx, fallback_filename: &str) -> String {
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
fn compute_stats(gpx: &gpx::Gpx) -> Result<(TraceStats, usize), String> {
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
fn extract_line_coordinates(gpx: &gpx::Gpx) -> Result<Vec<[f64; 2]>, String> {
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
fn build_geojson_feature(coords: Vec<[f64; 2]>, id: &str, name: &str) -> serde_json::Value {
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
// Gestion du registre traces.json
// ---------------------------------------------------------------------------

/// Charge le registre des traces depuis le fichier JSON.
/// Retourne un vecteur vide si le fichier n'existe pas ou est illisible.
fn load_registry(traces_path: &Path) -> Vec<TraceMetadata> {
    if !traces_path.exists() {
        return Vec::new();
    }

    match std::fs::read_to_string(traces_path) {
        Ok(content) => match serde_json::from_str::<Vec<TraceMetadata>>(&content) {
            Ok(registry) => registry,
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
fn save_registry(traces_path: &Path, registry: &[TraceMetadata]) -> Result<(), String> {
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
    let gpx_dir = get_gpx_dir(&mode_dir)?;
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

    // 7. Copier le fichier dans le dossier gpx/ avec un nom unique si nécessaire
    let raw_filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("trace.gpx");
    let safe_filename = sanitize_filename(raw_filename);
    let stored_filename = unique_filename(&gpx_dir, &safe_filename);
    let dest_path = gpx_dir.join(&stored_filename);

    std::fs::copy(&file_path, &dest_path)
        .map_err(|e| format!("Copie du fichier GPX : {}", e))?;

    // 8. Construire la métadonnée
    let id = uuid::Uuid::new_v4().to_string();
    let import_date = chrono::Utc::now().to_rfc3339();

    // 8bis. Générer la LineString GeoJSON et l'écrire dans geojson/{id}.geojson
    //       (après génération de l'UUID, avant la construction de TraceMetadata).
    let geojson_dir = get_geojson_dir(&mode_dir)?;
    let coords = extract_line_coordinates(&gpx)?;
    let feature = build_geojson_feature(coords, &id, &name);
    let geojson_path = geojson_dir.join(format!("{}.geojson", &id));
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

/// Supprime une trace (fichier GPX + entrée du registre) par son identifiant.
///
/// Supprime d'abord le fichier GPX sur disque (tolérant si absent), puis
/// retire l'entrée du registre et sauvegarde atomiquement traces.json.
#[tauri::command]
pub async fn delete_trace(app: tauri::AppHandle, trace_id: String) -> Result<(), String> {
    let mode_dir = get_mode_dir(&app)?;
    let gpx_dir = get_gpx_dir(&mode_dir)?;
    let traces_path = get_traces_path(&mode_dir);

    let mut registry = load_registry(&traces_path);

    // Recherche par id (clé unique UUID v4, pas par filename)
    let idx = registry
        .iter()
        .position(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable (id={})", trace_id))?;

    let filename = registry[idx].filename.clone();
    let trace_id_owned = registry[idx].id.clone();

    // 1) Supprimer le fichier GPX (tolérant si absent)
    let gpx_file = gpx_dir.join(&filename);
    if gpx_file.exists() {
        std::fs::remove_file(&gpx_file)
            .map_err(|e| format!("Suppression du fichier GPX : {}", e))?;
    }

    // 2) Supprimer le fichier LineString GeoJSON (tolérant si absent)
    let geojson_file = get_geojson_path(&mode_dir, &trace_id_owned);
    if geojson_file.exists() {
        std::fs::remove_file(&geojson_file)
            .map_err(|e| format!("Suppression du fichier GeoJSON : {}", e))?;
    }

    // 3) Retirer l'entrée du registre en mémoire
    registry.remove(idx);

    // 4) Sauvegarde atomique (tmp + rename via save_registry)
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
    gpx_dir: &Path,
    registry: &[TraceMetadata],
    trace_id: &str,
) -> Result<serde_json::Value, String> {
    let trace = registry
        .iter()
        .find(|t| t.id == trace_id)
        .ok_or_else(|| format!("Trace introuvable (id={})", trace_id))?;

    let gpx_file = gpx_dir.join(&trace.filename);
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

    // Écriture atomique du fichier geojson/{trace_id}.geojson.
    let geojson_dir = get_geojson_dir(mode_dir)?;
    let geojson_path = geojson_dir.join(format!("{}.geojson", trace_id));
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
/// Lit le fichier `{mode_dir}/geojson/{trace_id}.geojson` généré à l'import.
/// Si ce fichier manque (trace importée avant l'existence du dossier `geojson/`),
/// il est régénéré automatiquement depuis le GPX original, puis mis en cache.
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
    let gpx_dir = get_gpx_dir(&mode_dir)?;
    let traces_path = get_traces_path(&mode_dir);
    let registry = load_registry(&traces_path);
    let geometry = rebuild_geometry_from_gpx(&mode_dir, &gpx_dir, &registry, &trace_id)?;

    Ok(TraceGeometry {
        id: trace_id,
        geometry,
    })
}
