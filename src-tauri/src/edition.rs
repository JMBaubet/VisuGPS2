//! Module d'édition / post-montage des traces GPX pour VisuGPS2.
//!
//! Gère la persistance des fichiers de keyframes produits par le pré-calcul
//! (Mode 1) et des overrides de montage édités dans l'atelier (Mode 2) :
//!
//! - `{mode_dir}/keyframes/{traceId}_raw_keyframes.json`     → keyframes bruts
//!   (générés par l'algorithme de zone morte côté frontend).
//! - `{mode_dir}/keyframes/{traceId}_montage_overrides.json` → overrides de
//!   montage (caméra). Messages et POI sont anticipés (tableaux vides) pour
//!   une future phase 2 sans migration du schéma.
//!
//! Toutes les écritures sont atomiques (fichier temporaire + rename), par
//! cohérence avec `import_gpx.rs` (registre, geojson). Aucun calcul lourd
//! n'est effecté ici : le pré-calcul et le moteur de fusion (blending) vivent
//! côté frontend (ils nécessitent une instance Mapbox réelle avec terrain).

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Structures de données sérialisables (snake_case → miroir des interfaces TS)
// ---------------------------------------------------------------------------

/// État complet de la caméra à un instant donné (image clé).
///
/// `center` (lng/lat) est issu de l'algorithme de zone morte et n'est jamais
/// modifié par les overrides de montage (règle §6.2 de la spécification) :
/// seule la position reste pilotée automatiquement pour garder le traceur
/// dans le cadre. Les overrides agissent sur zoom/pitch/bearing.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CamState {
    pub lng: f64,
    pub lat: f64,
    pub zoom: f64,
    pub bearing: f64,
    pub pitch: f64,
}

/// Position du traceur (sujet suivi) à un instant donné.
/// L'altitude est relevée via `queryTerrainElevation` pendant le pré-calcul
/// (source primaire Mapbox), avec repli sur l'altitude GPX si la requête
/// renvoie `null`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct TraceurState {
    pub lng: f64,
    pub lat: f64,
    pub altitude: Option<f64>,
}

/// Image clé : enregistrement de l'état de la caméra et du traceur à un
/// instant `time` (en millisecondes depuis le départ).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Keyframe {
    pub time: u64,
    pub cam: CamState,
    pub traceur: TraceurState,
}

/// Dimensions du viewport de référence utilisé lors du pré-calcul.
///
/// Le pré-calcul force une résolution canonique (défaut 1920×1080) via
/// `map.resize()` afin que les calculs de zone morte (qui utilisent
/// `map.project()` → pixels) soient indépendants de l'écran réel. À la
/// lecture, `jumpTo` reproduit le même cadrage géographique sur n'importe
/// quel écran (vidéoprojecteur inclus) : un écran plus large révèle
/// simplement plus de contexte périphérique sans décaler le traceur.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ReferenceViewport {
    pub width: u32,
    pub height: u32,
}

/// Contenu du fichier `{traceId}_raw_keyframes.json`.
///
/// Produit par le pré-calcul (Mode 1), lu par le moteur de fusion puis par
/// l'atelier d'édition. `total_duration` et `total_distance` permettent de
/// graduer la timeline sans reparcourir les keyframes.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct RawKeyframesFile {
    pub trace_id: String,
    pub total_duration: u64,
    pub total_distance: f64,
    pub reference_viewport: ReferenceViewport,
    pub sample_rate: u64,
    pub keyframes: Vec<Keyframe>,
}

/// Paramètres d'un override de montage caméra.
///
/// Toutes les valeurs sont optionnelles : seules celles présentes sont
/// appliquées. Les valeurs absolues (`zoom`, `pitch`, `bearing`) écrasent la
/// valeur brute ; les offsets relatifs (`*_offset`) s'y ajoutent. Un override
/// peut combiner les deux ou n'utiliser que l'un des deux.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct OverrideParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoom: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pitch: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bearing: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoom_offset: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bearing_offset: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pitch_offset: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_multiplier: Option<f64>,
}

/// Override de montage : modification artistique ciblée sur une plage
/// temporelle `[start_time, end_time]` (en millisecondes).
///
/// `easing` lisse la transition entre la valeur brute et la valeur override
/// aux bords de la plage. `damping` (entre 0 et 1) ajoute un effet d'inertie.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Override {
    pub id: String,
    pub name: String,
    pub start_time: u64,
    pub end_time: u64,
    pub params: OverrideParams,
    pub easing: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damping: Option<f64>,
    /// Désactive temporairement un override pour comparer le rendu (§10.4).
    /// `false` par défaut si absent (compatibilité ascendante des fichiers).
    #[serde(default)]
    pub disabled: bool,
}

/// Contenu du fichier `{traceId}_montage_overrides.json`.
///
/// Les tableaux `messages` et `pois` sont anticipés vides : la phase 1 ne
/// couvre que les overrides caméra. Ils sont stockés en `serde_json::Value`
/// pour rester agnostiques du schéma futur (pas de struct à maintenir tant
/// que les fonctionnalités ne sont pas implémentées).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct MontageOverridesFile {
    pub overrides: Vec<Override>,
    #[serde(default)]
    pub messages: Vec<serde_json::Value>,
    #[serde(default)]
    pub pois: Vec<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Résolution des chemins
// ---------------------------------------------------------------------------

/// Retourne le dossier `keyframes/` à l'intérieur du mode actif.
/// Le dossier est créé s'il n'existe pas (cohérent avec `get_gpx_dir`).
fn get_keyframes_dir(mode_dir: &Path) -> Result<PathBuf, String> {
    let dir = mode_dir.join("keyframes");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Impossible de créer le dossier keyframes : {}", e))?;
    Ok(dir)
}

/// Construit le chemin d'un fichier de keyframes pour une trace donnée.
/// `suffix` vaut `"raw_keyframes"` ou `"montage_overrides"`.
fn keyframes_file_path(mode_dir: &Path, trace_id: &str, suffix: &str) -> PathBuf {
    mode_dir
        .join("keyframes")
        .join(format!("{}_{}.json", trace_id, suffix))
}

/// Écriture atomique d'un contenu JSON sérialisé (tmp + rename).
///
/// Même pattern que `save_registry` / l'écriture du geojson dans
/// `import_gpx.rs` : on évite tout fichier partiellement écrit en cas de
/// crash ou de coupure.
fn write_atomic_json(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    let tmp_path = path.with_extension("json.tmp");
    let content = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Sérialisation JSON : {}", e))?;
    std::fs::write(&tmp_path, content)
        .map_err(|e| format!("Écriture du fichier temporaire {:?} : {}", tmp_path, e))?;
    std::fs::rename(&tmp_path, path)
        .map_err(|e| format!("Renommage du fichier {:?} : {}", path, e))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Commandes Tauri
// ---------------------------------------------------------------------------

/// Indique si un fichier de keyframes bruts existe déjà pour la trace.
///
/// Permet à l'atelier d'édition de savoir si le pré-calcul a déjà été
/// effectué (cache disponible) ou s'il faut le lancer au clic « Éditer ».
#[tauri::command]
pub fn has_raw_keyframes(app: tauri::AppHandle, trace_id: String) -> bool {
    let Ok(mode_dir) = crate::import_gpx::get_mode_dir(&app) else {
        return false;
    };
    keyframes_file_path(&mode_dir, &trace_id, "raw_keyframes").exists()
}

/// Lit le fichier de keyframes bruts d'une trace.
///
/// Erreur si le fichier n'existe pas : le frontend doit appeler
/// `has_raw_keyframes` (ou `get_montage_overrides` qui ne plante pas) avant,
/// ou lancer le pré-calcul pour le générer.
#[tauri::command]
pub fn get_raw_keyframes(
    app: tauri::AppHandle,
    trace_id: String,
) -> Result<RawKeyframesFile, String> {
    let mode_dir = crate::import_gpx::get_mode_dir(&app)?;
    let path = keyframes_file_path(&mode_dir, &trace_id, "raw_keyframes");
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Lecture des keyframes bruts {:?} : {}", path, e))?;
    serde_json::from_str::<RawKeyframesFile>(&content)
        .map_err(|e| format!("Fichier keyframes brut invalide : {}", e))
}

/// Écrit le fichier de keyframes bruts d'une trace (écriture atomique).
///
/// Appelé par le frontend à l'issue du pré-calcul.
#[tauri::command]
pub fn save_raw_keyframes(
    app: tauri::AppHandle,
    trace_id: String,
    file: RawKeyframesFile,
) -> Result<(), String> {
    let mode_dir = crate::import_gpx::get_mode_dir(&app)?;
    get_keyframes_dir(&mode_dir)?;
    let path = keyframes_file_path(&mode_dir, &trace_id, "raw_keyframes");
    write_atomic_json(&path, &file)
}

/// Lit le fichier d'overrides de montage d'une trace.
///
/// Si le fichier n'existe pas (première édition), renvoie un fichier vierge
/// et **ne l'écrit pas** sur disque : l'écriture n'a lieu qu'à la première
/// modification de l'utilisateur (évite de créer des fichiers pour toute
/// trace simplement ouverte en édition).
#[tauri::command]
pub fn get_montage_overrides(
    app: tauri::AppHandle,
    trace_id: String,
) -> Result<MontageOverridesFile, String> {
    let mode_dir = crate::import_gpx::get_mode_dir(&app)?;
    let path = keyframes_file_path(&mode_dir, &trace_id, "montage_overrides");

    if !path.exists() {
        return Ok(MontageOverridesFile {
            overrides: Vec::new(),
            messages: Vec::new(),
            pois: Vec::new(),
        });
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Lecture des overrides {:?} : {}", path, e))?;
    serde_json::from_str::<MontageOverridesFile>(&content)
        .map_err(|e| format!("Fichier d'overrides invalide : {}", e))
}

/// Écrit le fichier d'overrides de montage d'une trace (écriture atomique).
///
/// Appelé à chaque modification d'override (ajout, édition, suppression,
/// activation/désactivation). La refusion des keyframes finaux se fait côté
/// frontend (opération quasi instantanée).
#[tauri::command]
pub fn save_montage_overrides(
    app: tauri::AppHandle,
    trace_id: String,
    file: MontageOverridesFile,
) -> Result<(), String> {
    let mode_dir = crate::import_gpx::get_mode_dir(&app)?;
    get_keyframes_dir(&mode_dir)?;
    let path = keyframes_file_path(&mode_dir, &trace_id, "montage_overrides");
    write_atomic_json(&path, &file)
}

/// Supprime tous les fichiers de keyframes d'une trace (cascade).
///
/// À appeler depuis le store frontend `traces.ts::supprimerTrace` après
/// `delete_trace`, afin que la suppression d'une trace purge aussi ses
/// fichiers d'édition. Les fichiers manquants sont ignorés silencieusement
/// (la trace n'a jamais été éditée). Le dossier `keyframes/` lui-même est
/// conservé (il peut contenir les fichiers d'autres traces).
#[tauri::command]
pub fn delete_keyframes(app: tauri::AppHandle, trace_id: String) -> Result<(), String> {
    let mode_dir = crate::import_gpx::get_mode_dir(&app)?;
    for suffix in ["raw_keyframes", "montage_overrides", "final_keyframes"] {
        let path = keyframes_file_path(&mode_dir, &trace_id, suffix);
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| format!("Suppression de {:?} : {}", path, e))?;
        }
    }
    Ok(())
}
