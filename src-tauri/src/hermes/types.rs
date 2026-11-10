//! Contrats IPC et codes d'erreur du client `hermes`.
//!
//! Les types sont sérialisés en **camelCase** pour le frontend, conformément à
//! la convention IPC du reste de l'application. Les réponses du serveur ne sont
//! pas uniformes : les routes `GET /datasets`, `/regions` et `/polygons`
//! utilisent du `snake_case` (`created_at`, `has_poly`, `geofabrik_id`…), tandis
//! que `/datasets/{id}/aux` et les routes de détection utilisent du `camelCase`.
//! Les champs concernés portent donc un `alias` de désérialisation : le client
//! accepte les deux écritures, et le frontend ne voit que du camelCase.
//!
//! Les commandes Tauri respectent `Result<T, String>`. Pour que le store puisse
//! choisir un message et une action (réveiller, ouvrir la vue Hermes, contacter
//! l'admin), la chaîne d'erreur commence par un **code machine** :
//! `"HERMES_OFFLINE: Connexion à hermes impossible…"`.

use serde::{Deserialize, Serialize};

/// Codes machine préfixant les erreurs renvoyées au frontend.
pub mod codes {
    /// Connexion refusée, DNS, ZeroTier absent.
    pub const OFFLINE: &str = "HERMES_OFFLINE";
    /// Délai dépassé.
    pub const TIMEOUT: &str = "HERMES_TIMEOUT";
    /// Ressource inconnue (404 générique).
    pub const NOT_FOUND: &str = "HERMES_NOT_FOUND";
    /// Artefacts auxiliaires absents (404 d'une route de détection).
    /// Réservé aux détections communes/tunnels (étapes 2 et 3).
    #[allow(dead_code)]
    pub const NOT_PREPARED: &str = "HERMES_NOT_PREPARED";
    /// Aucun dataset actif n'est sélectionné.
    /// Réservé aux détections communes/tunnels (étapes 2 et 3).
    #[allow(dead_code)]
    pub const NO_DATASET: &str = "HERMES_NO_DATASET";
    /// Conflit : build ORS ou préparation aux en cours (409).
    pub const CONFLICT: &str = "HERMES_CONFLICT";
    /// Action réservée à l'administrateur (401/403).
    pub const FORBIDDEN: &str = "HERMES_FORBIDDEN";
    /// Requête invalide (400/405/422, ou validation locale).
    pub const INVALID: &str = "HERMES_INVALID";
    /// Erreur interne ou réponse illisible (500).
    pub const SERVER: &str = "HERMES_SERVER";
}

/// Compose une erreur préfixée par son code machine.
pub fn coded(code: &str, message: impl Into<String>) -> String {
    format!("{}: {}", code, message.into())
}

// ─── Santé ────────────────────────────────────────────────────────────

/// Réponse de `GET /ping` et de `GET /ors/v2/health` : `{"status":"…"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HermesStatusResponse {
    pub status: String,
}

// ─── Datasets ─────────────────────────────────────────────────────────

/// Un dataset `hermes` (`GET /datasets` et `GET /datasets/{id}`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesDataset {
    pub id: String,
    /// Régions fusionnées (mode `regions`), ou `null` en mode `zones`.
    #[serde(default)]
    pub regions: Option<Vec<String>>,
    /// Zones custom (mode `zones`), ou `null` en mode `regions`.
    #[serde(default)]
    pub zones: Option<Vec<String>>,
    #[serde(default, alias = "created_at")]
    pub created_at: String,
    /// `preparing`, `ready` ou `failed`.
    pub status: String,
    #[serde(default, alias = "size_bytes")]
    pub size_bytes: u64,
    #[serde(default, alias = "ors_version")]
    pub ors_version: String,
    #[serde(default)]
    pub stale: bool,
    /// Dernière ligne de log d'un build échoué (`GET /datasets/{id}`).
    #[serde(default, alias = "last_log_tail")]
    pub last_log_tail: Option<String>,
}

/// Réponse `{id, status}` de `POST /datasets`, `GET /instances` et des
/// commandes start/stop d'instance ORS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesIdStatus {
    pub id: String,
    pub status: String,
}

/// Payload de `POST /datasets` : `regions` XOR `zones`.
///
/// Les champs `None` sont omis du JSON : le serveur applique son XOR strict et
/// refuse un mélange. La validation locale (voir `commands::hermes_create_dataset`)
/// intervient avant l'envoi pour un message immédiat.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesCreateDatasetRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regions: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zones: Option<Vec<String>>,
}

// ─── Régions ──────────────────────────────────────────────────────────

/// Une région indexée (`GET /regions`), avec son contour GeoJSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesRegion {
    pub id: String,
    /// `[lon_min, lat_min, lon_max, lat_max]`.
    pub bbox: [f64; 4],
    #[serde(alias = "has_poly")]
    pub has_poly: bool,
    /// Contour réel, ou polygone de la bbox si le serveur n'a pas le `.poly`.
    #[serde(default)]
    pub geojson: Option<serde_json::Value>,
    #[serde(default, alias = "geofabrik_id")]
    pub geofabrik_id: Option<String>,
    #[serde(default, alias = "updated_at")]
    pub updated_at: Option<String>,
}

// ─── Polygones custom ─────────────────────────────────────────────────

/// Une zone custom (`GET /polygons`), avec sa géométrie GeoJSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesPolygon {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    pub geometry: serde_json::Value,
    /// `[lon_min, lat_min, lon_max, lat_max]`.
    pub bbox: [f64; 4],
    #[serde(default, alias = "created_at")]
    pub created_at: String,
    #[serde(default, alias = "updated_at")]
    pub updated_at: String,
    #[serde(default)]
    pub revision: u32,
}

/// Payload de `POST /polygons` (`id` fourni) et `PUT /polygons/{id}` (`id`
/// omis, il est immuable et porté par le chemin).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesPolygonInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub geometry: serde_json::Value,
}

// ─── Artefacts auxiliaires ────────────────────────────────────────────

/// État d'un artefact auxiliaire (`communes.geojson`, `tunnels-logiques.json`,
/// `mm/`) dans `GET /datasets/{id}/aux`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesAuxFile {
    pub name: String,
    pub exists: bool,
    #[serde(default, alias = "size_bytes")]
    pub size_bytes: Option<u64>,
    #[serde(default, alias = "modified_at")]
    pub modified_at: Option<String>,
}

/// Réponse de `GET /datasets/{id}/aux`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesAuxStatus {
    #[serde(alias = "dataset_id")]
    pub dataset_id: String,
    pub communes: HermesAuxFile,
    pub tunnels: HermesAuxFile,
    pub mm: HermesAuxFile,
    #[serde(default)]
    pub preparing: bool,
    #[serde(default, alias = "started_at")]
    pub started_at: Option<String>,
    #[serde(default, alias = "last_error")]
    pub last_error: Option<String>,
    #[serde(default, alias = "loaded_in_memory")]
    pub loaded_in_memory: bool,
    #[serde(default, alias = "active_dataset")]
    pub active_dataset: Option<String>,
}

/// Réponse de `POST /datasets/{id}/aux/prepare` (202, tâche de fond).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesAuxPrepareResponse {
    #[serde(alias = "dataset_id")]
    pub dataset_id: String,
    pub status: String,
    #[serde(alias = "started_at")]
    pub started_at: String,
    #[serde(default)]
    pub message: Option<String>,
}

// ─── Réveil ───────────────────────────────────────────────────────────

/// Résultat de `hermes_wake`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesWakeResult {
    pub sent: bool,
    pub address: String,
    pub mac: String,
    pub port: u16,
}
