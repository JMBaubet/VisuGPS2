//! Commandes Tauri du client `hermes`.
//!
//! Toutes les E/S réseau passent par ce module : le frontend ne fait aucun
//! `fetch`, et aucun jeton administratif n'est stocké ni transmis. Les routes
//! de gestion des régions et Geofabrik (`POST/PUT/DELETE /regions`,
//! `/geofabrik`) restent hors périmètre. `POST /datasets/{id}/aux/prepare`
//! est exposé : le serveur l'autorise sans jeton et le store suit ensuite
//! `GET /datasets/{id}/aux`.

use std::time::Duration;

use serde_json::Value;

use super::client::{validate_id, HermesClient};
use super::config::{self, HermesConfig};
use super::types::{
    coded, codes, HermesAuxPrepareResponse, HermesAuxStatus, HermesCreateDatasetRequest,
    HermesDataset, HermesIdStatus, HermesPolygon, HermesPolygonInput, HermesRegion,
    HermesStatusResponse, HermesWakeResult,
};
use super::wake;

const PING_TIMEOUT: Duration = Duration::from_secs(2);
const ORS_HEALTH_TIMEOUT: Duration = Duration::from_secs(2);

/// Lit la configuration et construit le client.
async fn open(app: &tauri::AppHandle) -> Result<(HermesConfig, HermesClient), String> {
    let config = config::read_config(app).await;
    let client = HermesClient::new(&config)?;
    Ok((config, client))
}

// ─── Santé et réveil ──────────────────────────────────────────────────

/// `GET /ping` — état du contrôleur. Sans reprise : l'appelant veut une
/// réponse immédiate (statut du chip, avant une détection).
#[tauri::command]
pub async fn hermes_ping(app: tauri::AppHandle) -> Result<HermesStatusResponse, String> {
    let (_, client) = open(&app).await?;
    client
        .get_once::<HermesStatusResponse>("/ping", PING_TIMEOUT)
        .await
}

/// `GET {orsBaseUrl}/health` — santé du moteur ORS hébergé par hermes.
#[tauri::command]
pub async fn hermes_ors_health(app: tauri::AppHandle) -> Result<HermesStatusResponse, String> {
    let (_, client) = open(&app).await?;
    let url = client.ors_health_url();
    client
        .get_absolute_once::<HermesStatusResponse>(&url, ORS_HEALTH_TIMEOUT)
        .await
}

/// Envoie le paquet magique Wake-on-LAN. Le polling de disponibilité est
/// piloté par le store (ping toutes les 3 s, budget `delaiMaxSec`).
#[tauri::command]
pub async fn hermes_wake(app: tauri::AppHandle) -> Result<HermesWakeResult, String> {
    let config = config::read_config(&app).await;
    wake::send_magic_packet(&config.wake_address, config.wake_port, &config.wake_mac)
        .map_err(|error| coded(codes::INVALID, error))?;
    Ok(HermesWakeResult {
        sent: true,
        address: config.wake_address,
        mac: config.wake_mac,
        port: config.wake_port,
    })
}

// ─── Datasets ─────────────────────────────────────────────────────────

/// `GET /datasets`.
#[tauri::command]
pub async fn hermes_list_datasets(
    app: tauri::AppHandle,
) -> Result<Vec<HermesDataset>, String> {
    let (config, client) = open(&app).await?;
    client
        .get::<Vec<HermesDataset>>("/datasets", config.request_timeout)
        .await
}

/// `GET /datasets/{id}` — utilisé par le polling de build.
#[tauri::command]
pub async fn hermes_get_dataset(
    app: tauri::AppHandle,
    dataset_id: String,
) -> Result<HermesDataset, String> {
    validate_id(&dataset_id, "dataset")?;
    let (config, client) = open(&app).await?;
    client
        .get::<HermesDataset>(
            &format!("/datasets/{}", dataset_id),
            config.request_timeout,
        )
        .await
}

/// `POST /datasets` — crée un dataset en mode `regions` **XOR** `zones`.
///
/// La validation locale reproduit le XOR du serveur pour un message immédiat ;
/// les limites (4 régions, 100 zones, 50 000 km²) restent l'autorité du serveur
/// et sont restituées telles quelles.
#[tauri::command]
pub async fn hermes_create_dataset(
    app: tauri::AppHandle,
    name: Option<String>,
    regions: Option<Vec<String>>,
    zones: Option<Vec<String>>,
) -> Result<HermesIdStatus, String> {
    let regions = normalize_ids(regions, "région")?;
    let zones = normalize_ids(zones, "zone")?;

    match (&regions, &zones) {
        (Some(regions), None) if !regions.is_empty() => {}
        (None, Some(zones)) if !zones.is_empty() => {}
        (Some(_), Some(_)) => {
            return Err(coded(
                codes::INVALID,
                "Régions et zones ne peuvent pas être fournies ensemble.",
            ));
        }
        _ => {
            return Err(coded(
                codes::INVALID,
                "Fournissez une liste non vide de régions ou de zones.",
            ));
        }
    }

    let name = name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let (config, client) = open(&app).await?;
    client
        .post::<HermesIdStatus, _>(
            "/datasets",
            &HermesCreateDatasetRequest {
                name,
                regions,
                zones,
            },
            config.request_timeout,
        )
        .await
}

/// `DELETE /datasets/{id}` — irréversible, la confirmation est à la charge de
/// l'IHM.
#[tauri::command]
pub async fn hermes_delete_dataset(
    app: tauri::AppHandle,
    dataset_id: String,
) -> Result<(), String> {
    validate_id(&dataset_id, "dataset")?;
    let (config, client) = open(&app).await?;
    client
        .delete(&format!("/datasets/{}", dataset_id), config.request_timeout)
        .await
}

/// `GET /datasets/{id}/aux` — artefacts communes/tunnels/mm et diagnostic de
/// l'index chargé en mémoire.
#[tauri::command]
pub async fn hermes_aux_status(
    app: tauri::AppHandle,
    dataset_id: String,
) -> Result<HermesAuxStatus, String> {
    validate_id(&dataset_id, "dataset")?;
    let (config, client) = open(&app).await?;
    client
        .get::<HermesAuxStatus>(
            &format!("/datasets/{}/aux", dataset_id),
            config.request_timeout,
        )
        .await
}

/// `POST /datasets/{id}/aux/prepare` — lance la génération des artefacts aux.
///
/// Le serveur répond 202 et poursuit en tâche de fond : le store suit ensuite
/// `GET /datasets/{id}/aux`. `force` régénère les artefacts existants,
/// `skip_mm` saute la génération du dossier `mm/`.
#[tauri::command]
pub async fn hermes_prepare_aux(
    app: tauri::AppHandle,
    dataset_id: String,
    force: bool,
    skip_mm: bool,
) -> Result<HermesAuxPrepareResponse, String> {
    validate_id(&dataset_id, "dataset")?;
    let (config, client) = open(&app).await?;
    let body = serde_json::json!({
        "force": force,
        "skipMm": skip_mm,
    });
    client
        .post(
            &format!("/datasets/{}/aux/prepare", dataset_id),
            &body,
            config.request_timeout,
        )
        .await
}

// ─── Instances ORS ────────────────────────────────────────────────────

/// `GET /instances` — 0 ou 1 instance active.
#[tauri::command]
pub async fn hermes_list_instances(
    app: tauri::AppHandle,
) -> Result<Vec<HermesIdStatus>, String> {
    let (config, client) = open(&app).await?;
    client
        .get::<Vec<HermesIdStatus>>("/instances", config.request_timeout)
        .await
}

/// `POST /instances/{id}/start` — `starting` ne vaut pas `ready` : le store
/// interroge ensuite `hermes_ors_health`.
#[tauri::command]
pub async fn hermes_start_instance(
    app: tauri::AppHandle,
    dataset_id: String,
) -> Result<HermesIdStatus, String> {
    validate_id(&dataset_id, "dataset")?;
    let (config, client) = open(&app).await?;
    client
        .post::<HermesIdStatus, _>(
            &format!("/instances/{}/start", dataset_id),
            &serde_json::json!({}),
            config.request_timeout,
        )
        .await
}

/// `POST /instances/{id}/stop` — arrête ORS et décharge les index communes et
/// tunnels (le prochain appel de détection recharge ~2 s).
#[tauri::command]
pub async fn hermes_stop_instance(
    app: tauri::AppHandle,
    dataset_id: String,
) -> Result<HermesIdStatus, String> {
    validate_id(&dataset_id, "dataset")?;
    let (config, client) = open(&app).await?;
    client
        .post::<HermesIdStatus, _>(
            &format!("/instances/{}/stop", dataset_id),
            &serde_json::json!({}),
            config.request_timeout,
        )
        .await
}

// ─── Régions (lecture seule) ──────────────────────────────────────────

/// `GET /regions` — les régions sont gérées par l'administrateur ; M1 ne les
/// modifie pas.
#[tauri::command]
pub async fn hermes_list_regions(
    app: tauri::AppHandle,
) -> Result<Vec<HermesRegion>, String> {
    let (config, client) = open(&app).await?;
    client
        .get::<Vec<HermesRegion>>("/regions", config.request_timeout)
        .await
}

// ─── Polygones custom ────────────────────────────────────────────────

/// `GET /polygons`.
#[tauri::command]
pub async fn hermes_list_polygons(
    app: tauri::AppHandle,
) -> Result<Vec<HermesPolygon>, String> {
    let (config, client) = open(&app).await?;
    client
        .get::<Vec<HermesPolygon>>("/polygons", config.request_timeout)
        .await
}

/// `GET /polygons/{id}`.
#[tauri::command]
pub async fn hermes_get_polygon(
    app: tauri::AppHandle,
    polygon_id: String,
) -> Result<HermesPolygon, String> {
    validate_id(&polygon_id, "polygone")?;
    let (config, client) = open(&app).await?;
    client
        .get::<HermesPolygon>(
            &format!("/polygons/{}", polygon_id),
            config.request_timeout,
        )
        .await
}

/// `POST /polygons` — crée une zone custom. L'identifiant est un slug fourni
/// par l'IHM ; `index` est réservé par le serveur.
#[tauri::command]
pub async fn hermes_create_polygon(
    app: tauri::AppHandle,
    id: String,
    name: String,
    description: Option<String>,
    color: Option<String>,
    geometry: Value,
) -> Result<HermesPolygon, String> {
    validate_id(&id, "polygone")?;
    if id == "index" {
        return Err(coded(
            codes::INVALID,
            "L'identifiant « index » est réservé par le serveur.",
        ));
    }
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(coded(
            codes::INVALID,
            "Le nom du polygone est obligatoire.",
        ));
    }
    validate_polygon_geometry(&geometry)?;

    let (config, client) = open(&app).await?;
    client
        .post::<HermesPolygon, _>(
            "/polygons",
            &HermesPolygonInput {
                id: Some(id),
                name,
                description: clean_optional(description),
                color: clean_optional(color),
                geometry,
            },
            config.request_timeout,
        )
        .await
}

/// `PUT /polygons/{id}` — remplacement complet ; `id` et `created_at` sont
/// immuables côté serveur.
#[tauri::command]
pub async fn hermes_update_polygon(
    app: tauri::AppHandle,
    polygon_id: String,
    name: String,
    description: Option<String>,
    color: Option<String>,
    geometry: Value,
) -> Result<HermesPolygon, String> {
    validate_id(&polygon_id, "polygone")?;
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(coded(
            codes::INVALID,
            "Le nom du polygone est obligatoire.",
        ));
    }
    validate_polygon_geometry(&geometry)?;

    let (config, client) = open(&app).await?;
    client
        .put::<HermesPolygon, _>(
            &format!("/polygons/{}", polygon_id),
            &HermesPolygonInput {
                id: None,
                name,
                description: clean_optional(description),
                color: clean_optional(color),
                geometry,
            },
            config.request_timeout,
        )
        .await
}

/// `DELETE /polygons/{id}`.
#[tauri::command]
pub async fn hermes_delete_polygon(
    app: tauri::AppHandle,
    polygon_id: String,
) -> Result<(), String> {
    validate_id(&polygon_id, "polygone")?;
    let (config, client) = open(&app).await?;
    client
        .delete(&format!("/polygons/{}", polygon_id), config.request_timeout)
        .await
}

// ─── Helpers ─────────────────────────────────────────────────────────

fn normalize_ids(ids: Option<Vec<String>>, what: &str) -> Result<Option<Vec<String>>, String> {
    let Some(ids) = ids else {
        return Ok(None);
    };
    let mut normalized = Vec::with_capacity(ids.len());
    for id in ids {
        let id = id.trim().to_string();
        validate_id(&id, what)?;
        normalized.push(id);
    }
    Ok(Some(normalized))
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Garde-fou local : le serveur n'accepte que `Polygon` (pas de `MultiPolygon`,
/// pas de trous). La validation complète (sommets, fermeture, précision)
/// reste côté serveur.
fn validate_polygon_geometry(geometry: &Value) -> Result<(), String> {
    if geometry.get("type").and_then(Value::as_str) != Some("Polygon") {
        return Err(coded(
            codes::INVALID,
            "La géométrie doit être un Polygon GeoJSON.",
        ));
    }
    Ok(())
}
