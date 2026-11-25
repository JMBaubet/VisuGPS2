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
    /// Connexion refusée, DNS, Tailscale absent.
    pub const OFFLINE: &str = "HERMES_OFFLINE";
    /// Délai dépassé.
    pub const TIMEOUT: &str = "HERMES_TIMEOUT";
    /// Ressource inconnue (404 générique).
    pub const NOT_FOUND: &str = "HERMES_NOT_FOUND";
    /// Artefacts auxiliaires absents (404 d'une route de détection).
    pub const NOT_PREPARED: &str = "HERMES_NOT_PREPARED";
    /// Aucun dataset actif n'est sélectionné.
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
    /// Nom d'affichage du dataset (facultatif : absent des serveurs < v1.11).
    #[serde(default)]
    pub name: Option<String>,
    /// Régions fusionnées (mode `regions`), ou `null` en mode `zones`.
    #[serde(default)]
    pub regions: Option<Vec<String>>,
    /// Zones custom (mode `zones`), ou `null` en mode `regions`.
    #[serde(default)]
    pub zones: Option<Vec<String>>,
    #[serde(default, alias = "created_at")]
    pub created_at: String,
    /// Champ historique : `preparing`, `ready` ou `failed`.
    pub status: String,
    /// État de référence v1.9 : `initializing`, `building`, `ready`,
    /// `unavailable`, `degradedCommunes`, `degradedTunnels`, `degradedAux`.
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default, alias = "state_label")]
    pub state_label: Option<String>,
    /// Phase courante du build (ex. `pbf`).
    #[serde(default)]
    pub phase: Option<String>,
    /// Présence des artefacts `pbf`, `ors`, `communes`, `tunnels`, `mm`.
    #[serde(default)]
    pub artifacts: Option<serde_json::Value>,
    #[serde(default, alias = "run_id")]
    pub run_id: Option<String>,
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

/// État du dataset servi par Hermes (`GET /serving`).
///
/// `state` vaut `none`, `waiting`, `starting`, `active`, `stopped` ou
/// `failed`. La sélection est persistée côté serveur dans `serving.json` ;
/// `PUT /serving/{id}` la modifie et démarre ORS, `DELETE /serving` l'efface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesServingStatus {
    #[serde(default, alias = "dataset_id")]
    pub dataset_id: Option<String>,
    #[serde(default)]
    pub state: String,
    #[serde(default, alias = "state_label")]
    pub state_label: Option<String>,
    #[serde(default, alias = "dataset_state")]
    pub dataset_state: Option<String>,
    #[serde(default)]
    pub since: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
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
    pub geometry: serde_json::Value,
}

// ─── Artefacts auxiliaires ────────────────────────────────────────────

/// État d'un artefact auxiliaire (`communes.geojson`, `tunnels-logiques.json`,
/// `mm/`) dans `GET /datasets/{id}/aux`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesAuxFile {
    pub name: String,
    /// `pending`, `building`, `ready` ou `failed` (contrat v1.10).
    #[serde(default)]
    pub state: String,
    pub exists: bool,
    #[serde(default, alias = "size_bytes")]
    pub size_bytes: Option<u64>,
    #[serde(default, alias = "modified_at")]
    pub modified_at: Option<String>,
    #[serde(default, alias = "completed_at")]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default, alias = "run_id")]
    pub run_id: Option<String>,
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

// ─── Détection des communes ───────────────────────────────────────────

/// Réglages de `POST /communes/detect` (objet `params` du contrat).
///
/// La géométrie est traitée par le serveur, qui applique ses propres règles :
/// seuls les réglages qui gardent un sens côté client sont transmis. L'écart de
/// fusion des passages contigus (`gapMaxM`) reste au défaut du serveur (50 m).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesCommunesParams {
    /// Niveau administratif OSM forcé (`"8"` = communes en France, Espagne,
    /// Italie, Suisse) ; `None` laisse le serveur appliquer son mapping par
    /// pays.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_level: Option<String>,
    /// Longueur minimale d'un passage retenu, en kilomètres.
    pub min_segment_km: f64,
}

/// Payload de `POST /communes/detect` (contrat v1.10).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesCommunesDetectRequest {
    pub trace_id: String,
    pub dataset_id: String,
    /// Points `[lon, lat]` de la trace, dans l'ordre chronologique.
    pub points: Vec<[f64; 2]>,
    pub params: HermesCommunesParams,
}

/// Un passage dans une commune, tel que le serveur le renvoie.
///
/// La route de détection répond en `camelCase` (`kmEntree`,
/// `nbPassagesTotal`…) ; les alias `snake_case` gardent le client tolérant,
/// comme pour les autres routes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesCommunePassage {
    pub commune: String,
    pub passage: u32,
    #[serde(alias = "nb_passages_total")]
    pub nb_passages_total: u32,
    #[serde(alias = "km_entree")]
    pub km_entree: f64,
    #[serde(alias = "km_sortie")]
    pub km_sortie: f64,
    #[serde(alias = "longueur_km")]
    pub longueur_km: f64,
    /// Deux passages de communes différentes se chevauchent de plus de 1 m :
    /// la frontière OSM est imprécise à cet endroit.
    #[serde(default)]
    pub ambiguous: bool,
}

/// Réponse de `POST /communes/detect` : `passages` vide est un résultat, y
/// compris quand la trace sort de la couverture du dataset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesCommunesDetectResponse {
    #[serde(alias = "trace_id")]
    pub trace_id: String,
    #[serde(alias = "dataset_id")]
    pub dataset_id: String,
    #[serde(default, alias = "detected_at")]
    pub detected_at: String,
    pub passages: Vec<HermesCommunePassage>,
}

// ─── Détection des tunnels ────────────────────────────────────────────

/// Réglages de `POST /tunnels/detect` (objet `params` du contrat).
///
/// Le contrôle altimétrique reste **désactivé** côté client : le serveur ne
/// l'utilise que pour annoter un tunnel déjà apparié, et `trace.geojson` ne
/// porte pas d'altitude. Les deux champs sont transmis explicitement pour que le
/// fichier `tunnels.json` documente la requête réellement envoyée.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesTunnelParams {
    /// Longueur minimale d'un tunnel retenu, en mètres.
    pub min_length_m: f64,
    /// Contrôle altimétrique optionnel (toujours `false` côté client).
    pub altimetric_check: bool,
    /// Tolérance du contrôle altimétrique, en mètres.
    pub altimetric_tolerance_m: f64,
}

/// Payload de `POST /tunnels/detect` (contrat v1.10).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesTunnelsDetectRequest {
    pub trace_id: String,
    pub dataset_id: String,
    /// Points `[lon, lat]` de la trace, dans l'ordre chronologique.
    pub points: Vec<[f64; 2]>,
    pub params: HermesTunnelParams,
}

/// Un tunnel tel que le serveur le renvoie.
///
/// `way_ids` porte les ways OSM couvertes par le tunnel logique (union-find
/// côté serveur) ; `altimetric_ok` reste `None` tant que le contrôle
/// altimétrique n'est pas activé.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesTunnel {
    pub id: String,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(alias = "start_km")]
    pub start_km: f64,
    #[serde(alias = "end_km")]
    pub end_km: f64,
    #[serde(default, alias = "length_m")]
    pub length_m: f64,
    #[serde(default, alias = "way_ids")]
    pub way_ids: Vec<String>,
    #[serde(default, alias = "altimetric_ok")]
    pub altimetric_ok: Option<bool>,
}

/// Réponse de `POST /tunnels/detect` : `tunnels` vide est un résultat, y
/// compris quand la trace sort de la couverture du dataset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesTunnelsDetectResponse {
    #[serde(default)]
    pub version: u32,
    #[serde(default, alias = "trace_id")]
    pub trace_id: String,
    #[serde(default, alias = "dataset_id")]
    pub dataset_id: String,
    #[serde(default, alias = "detected_at")]
    pub detected_at: String,
    #[serde(default)]
    pub tunnels: Vec<HermesTunnel>,
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
