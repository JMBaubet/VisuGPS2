//! Types du module Tunnels.
//!
//! Ils sont sérialisés en **camelCase** : `tunnels.json` est un contrat de
//! sortie consommé par la vue, et les commandes Tauri suivent la convention IPC
//! du reste de l'application.
//!
//! Le fichier est à la fois **état restaurable** et sortie d'analyse :
//! - `tunnels` porte les tronçons détectés ou créés manuellement ;
//! - `groupId` regroupe des tronçons fusionnés **sans les rendre continus** :
//!   la géométrie réelle de chaque tronçon est conservée, l'écart entre deux
//!   tronçons reste du terrain découvert ;
//! - `params` et `pbfFiles` documentent la détection qui a produit le fichier.
//!
//! Les statuts de validation (`waiting_validate` / `success`) ne sont **pas**
//! stockés ici : ils vivent dans le registre des traces, comme
//! `altimetrie_validated`, afin que l'icône du circuit puisse les lire sans
//! ouvrir le fichier.

use serde::{Deserialize, Serialize};

/// Version du format du fichier. Une version inconnue est ignorée par
/// `file::load_file`, qui laisse l'appelant relancer la détection.
pub const FILE_VERSION: u32 = 1;

// ─── Statuts du registre ──────────────────────────────────────────────

/// Fichier présent avec au moins un tunnel, pas encore validé par l'opérateur.
pub const STATUS_WAITING_VALIDATE: &str = "waiting_validate";
/// Fichier validé par une sauvegarde opérateur, ou détection sans tunnel.
pub const STATUS_SUCCESS: &str = "success";
/// La détection elle-même a échoué (PBF illisible, aucune région, etc.).
pub const STATUS_ERROR: &str = "error";
/// Le dossier OSM n'est pas configuré ou n'est pas lisible.
pub const STATUS_NOT_CONFIGURED: &str = "not_configured";
/// Les PBF requis pour cette trace sont absents ou n'ont pas pu être obtenus.
pub const STATUS_MISSING_PBF: &str = "missing_pbf";

// ─── États de préparation ─────────────────────────────────────────────

/// Le module est désactivé par `Tunnels.Support.actif`.
pub const READINESS_DISABLED: &str = "disabled";
/// Aucun dossier OSM n'est configuré ou il n'est pas lisible.
pub const READINESS_NOT_CONFIGURED: &str = "not_configured";
/// `regions.json` est absent ou illisible : il faut le produire.
pub const READINESS_NEEDS_REGIONS: &str = "needs_regions";
/// Aucune région Geofabrik ne couvre la bbox de la trace.
pub const READINESS_NO_REGION: &str = "no_region";
/// Une ou plusieurs PBF requises sont absentes.
pub const READINESS_MISSING_PBF: &str = "missing_pbf";
/// Toutes les ressources sont présentes.
pub const READINESS_READY: &str = "ready";

// ─── Paramètres ───────────────────────────────────────────────────────

/// Paramètres du pipeline de détection. Leurs chemins de configuration sont
/// `Tunnels.Detection.*` dans `settings.default.toml` ; les défauts ci-dessous
/// sont le miroir du prototype validé.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelParams {
    /// Tolérance de distance entre un point GPX et un segment OSM (m).
    pub distance_tolerance_m: f64,
    /// Tolérance d'écart angulaire entre le cap GPX et le segment OSM (°).
    pub angle_tolerance_deg: f64,
    /// Marge ajoutée à la bbox de la trace pour l'extraction OSM (m).
    pub bbox_margin_m: f64,
    /// Fenêtre glissante de calcul du cap GPX (m).
    pub bearing_window_m: f64,
    /// Nombre minimal de points consécutifs appariés pour retenir un tunnel.
    pub min_run_length: usize,
}

impl Default for TunnelParams {
    fn default() -> Self {
        Self {
            distance_tolerance_m: 20.0,
            angle_tolerance_deg: 25.0,
            bbox_margin_m: 500.0,
            bearing_window_m: 30.0,
            min_run_length: 2,
        }
    }
}

/// Support OSM : dossier de la clé USB et activation du module.
///
/// `pbf_directory` reste la valeur brute du paramètre ; la résolution et la
/// vérification du dossier appartiennent à `commands`.
#[derive(Debug, Clone, PartialEq)]
pub struct TunnelSupport {
    pub actif: bool,
    /// Plafond de téléchargement d'un PBF (Mo) ; 0 = illimité.
    pub taille_max_pbf_mb: u64,
    pub pbf_directory: Option<String>,
}

impl Default for TunnelSupport {
    fn default() -> Self {
        Self {
            actif: true,
            taille_max_pbf_mb: 500,
            pbf_directory: None,
        }
    }
}

// ─── Fichier tunnels.json ─────────────────────────────────────────────

/// Racine du fichier `traces/{trace_id}/tunnels.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelFile {
    pub version: u32,
    pub trace_id: String,
    /// Date de la détection qui a créé le fichier ; `None` pour un fichier
    /// créé uniquement par des ajouts manuels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_at: Option<String>,
    /// Date de la dernière écriture (détection ou sauvegarde), ISO 8601.
    pub updated_at: String,
    /// Fichiers PBF effectivement analysés lors de la détection.
    #[serde(default)]
    pub pbf_files: Vec<PbfFileRef>,
    pub params: TunnelParams,
    /// Tunnels triés par `startKm` croissant.
    #[serde(default)]
    pub tunnels: Vec<Tunnel>,
}

/// Référence d'un PBF utilisé, pour la traçabilité du fichier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PbfFileRef {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<String>,
}

/// Un tunnel, tronçon réel de la trace.
///
/// `groupId` identifie un groupe de tronçons fusionnés : chaque tronçon garde
/// ses bornes propres. Une fusion ne crée donc jamais de tunnel continu sur
/// l'écart entre deux tronçons.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tunnel {
    /// Identifiant stable pour la sélection et les opérations de l'IHM.
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub way_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highway: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tunnel_type: Option<String>,
    pub start_km: f64,
    pub end_km: f64,
    /// Longueur dérivée, recalculée côté Rust à chaque sauvegarde.
    pub length_m: f64,
    /// `"osm"` pour une détection, `"manual"` pour une création opérateur.
    pub detection_method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_distance_m: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_angle_diff_deg: Option<f64>,
    /// Groupe de fusion ; absent pour un tunnel isolé.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
}

// ─── Index Geofabrik ──────────────────────────────────────────────────

/// Entrée de `regions.json`, produite depuis l'index Geofabrik.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub parent: String,
    /// Bbox `[minLon, minLat, maxLon, maxLat]`.
    pub bbox: [f64; 4],
    pub pbf_filename: String,
    pub pbf_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_mb: Option<f64>,
}

/// Racine de `regions.json`, sur le support OSM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionsFile {
    /// Date de génération, ISO 8601.
    pub version: String,
    pub regions: Vec<RegionEntry>,
}

// ─── États exposés à l'IHM ────────────────────────────────────────────

/// État du support OSM, lu par la carte du circuit sans ouvrir de fichier.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelSupportStatus {
    pub active: bool,
    pub configured: bool,
    pub folder: Option<String>,
    pub readable: bool,
    pub regions_present: bool,
    pub regions_version: Option<String>,
    pub pbf_count: usize,
}

/// Une région requise par la trace, avec sa présence sur le support.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelRequirementRegion {
    pub id: String,
    pub name: String,
    pub pbf_filename: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_mb: Option<f64>,
    pub present: bool,
    /// PBF manquant dont la taille distante dépasse le plafond configuré.
    pub too_large: bool,
}

/// Ressources OSM nécessaires à la détection d'une trace.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelRequirements {
    pub readiness: String,
    pub folder: Option<String>,
    pub regions: Vec<TunnelRequirementRegion>,
    pub missing_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Résultat d'une préparation ou d'une détection.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelPrepareResult {
    pub file: TunnelFile,
    /// `waiting_validate` ou `success`, tel que posé dans le registre.
    pub status: String,
}

/// Progression d'une préparation, émise par l'événement `tunnels-progress`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelProgress {
    pub trace_id: String,
    /// `regions`, `pbf` ou `detect`.
    pub stage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// 0–100 ; 0 lorsque la taille totale est inconnue.
    pub percent: f64,
}
