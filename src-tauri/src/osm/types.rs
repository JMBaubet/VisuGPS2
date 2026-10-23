//! Contrats IPC de la gestion des données OSM.
//!
//! Les types sont sérialisés en camelCase pour le frontend. Les états sont des
//! chaînes, comme pour les modules Tunnels et Communes, afin de rester
//! compatibles avec les valeurs déjà exposées par le registre.

use serde::Serialize;

/// Aucun dossier OSM n'est configuré.
pub const STATE_NOT_CONFIGURED: &str = "not_configured";
/// Le dossier configuré n'est pas lisible (clé USB absente, permissions…).
pub const STATE_USB_MISSING: &str = "usb_missing";
/// Le dossier est lisible mais ne contient aucun PBF.
pub const STATE_EMPTY: &str = "empty";
/// Tous les fichiers sont plus récents que la durée de renouvellement.
pub const STATE_GREEN: &str = "green";
/// Au moins un fichier atteint la durée de renouvellement.
pub const STATE_YELLOW: &str = "yellow";
/// Au moins un fichier atteint 90 % de sa durée maximale de validité.
pub const STATE_ORANGE: &str = "orange";

pub const FILE_STATE_GREEN: &str = "green";
pub const FILE_STATE_YELLOW: &str = "yellow";
pub const FILE_STATE_ORANGE: &str = "orange";

pub const DATE_SOURCE_CREATED: &str = "created";
pub const DATE_SOURCE_MODIFIED: &str = "modified";
pub const DATE_SOURCE_UNKNOWN: &str = "unknown";
/// Date extraite de l'en-tête PBF (`osmosis_replication_timestamp`).
pub const DATE_SOURCE_PBF: &str = "pbf";

/// État global du support OSM, consommé par l'icône de la toolbar.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsmSupportOverview {
    pub configured: bool,
    pub readable: bool,
    pub usb_missing: bool,
    pub folder: Option<String>,
    pub regions_present: bool,
    pub regions_version: Option<String>,
    pub free_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub max_pbf_mb: u64,
    pub max_validity_months: u32,
    pub renewal_weeks: u32,
    /// `not_configured`, `usb_missing`, `empty`, `green`, `yellow` ou `orange`.
    pub state: String,
    pub files: Vec<OsmFileStatus>,
    pub total_size_bytes: u64,
    pub message: Option<String>,
}

/// État d'un PBF présent sur le support.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsmFileStatus {
    pub filename: String,
    pub region_id: Option<String>,
    pub region_name: Option<String>,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub continent: Option<String>,
    pub continent_name: Option<String>,
    pub size_bytes: u64,
    pub created_at: Option<String>,
    /// `created`, `modified` ou `unknown`.
    pub date_source: String,
    pub age_days: i64,
    pub renewal_days: i64,
    /// Positif = jours restants, négatif = jours de retard.
    pub days_remaining: i64,
    /// Jours restants avant la fin de vie (négatif si dépassée).
    pub validity_days_remaining: Option<i64>,
    /// `green`, `yellow` ou `orange`.
    pub state: String,
}

/// Catalogue Geofabrik chargé depuis `regions.json`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsmRegionCatalog {
    pub present: bool,
    pub version: Option<String>,
    pub regions: Vec<OsmRegionOption>,
}

/// Une région sélectionnable dans la fenêtre de gestion.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsmRegionOption {
    pub id: String,
    pub name: String,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub continent: Option<String>,
    pub continent_name: Option<String>,
    pub pbf_filename: String,
    pub size_mb: Option<f64>,
    pub present: bool,
    pub local_size_bytes: Option<u64>,
    pub local_created_at: Option<String>,
}

/// Taille distante (HEAD) et espace disponible au moment du choix.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsmRegionProbe {
    pub bytes: Option<u64>,
    pub available_bytes: Option<u64>,
}

/// Géométrie GeoJSON simplifiée d'une région, pour l'aperçu sur la carte.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsmRegionGeometry {
    pub region_id: String,
    pub bbox: [f64; 4],
    pub geometry: serde_json::Value,
    /// Vrai lorsque la géométrie réelle est absente et que la bbox est utilisée.
    pub approximate: bool,
}

/// Géométrie d'une région dont le PBF est présent, avec son état de fraîcheur.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsmPresentRegionGeometry {
    pub region_id: String,
    /// `green`, `yellow` ou `orange`.
    pub state: String,
    pub bbox: [f64; 4],
    pub geometry: serde_json::Value,
    pub approximate: bool,
}

/// Résultat d'un téléchargement ou d'un renouvellement.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsmDownloadResult {
    pub filename: String,
    pub size_bytes: u64,
}

/// Progression d'un téléchargement ou du rafraîchissement du catalogue,
/// émise par l'événement `osm-progress`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsmProgress {
    /// `catalog` ou `download`.
    pub stage: String,
    pub region_id: Option<String>,
    pub region_name: Option<String>,
    pub filename: Option<String>,
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// 0–100 ; 0 lorsque la taille totale est inconnue.
    pub percent: f64,
}
