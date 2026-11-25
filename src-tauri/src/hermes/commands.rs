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
use super::system_ping;
use super::types::{
    coded, codes, HermesAuxPrepareResponse, HermesAuxStatus, HermesCreateDatasetRequest,
    HermesDataset, HermesIdStatus, HermesPolygon, HermesPolygonInput, HermesRegion,
    HermesServingStatus, HermesStatusResponse, HermesWakeResult,
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

/// Ping **système** (ICMP) de l'hôte hermes. Il teste la machine et le réseau,
/// indépendamment du service `hermes-api` : le store l'exécute périodiquement
/// pendant toute la durée de l'application.
#[tauri::command]
pub async fn hermes_system_ping(
    app: tauri::AppHandle,
) -> Result<HermesStatusResponse, String> {
    let config = config::read_config(&app).await;
    let host = system_ping::host_from_base_url(&config.base_url)
        .ok_or_else(|| coded(codes::INVALID, "Hôte hermes introuvable dans l'URL."))?;
    system_ping::ping_once(&host)
        .await
        .map_err(|error| coded(codes::OFFLINE, error))?;
    Ok(HermesStatusResponse {
        status: "ok".to_string(),
    })
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

/// `GET /serving` — dataset servi et état de l'instance ORS.
#[tauri::command]
pub async fn hermes_get_serving(
    app: tauri::AppHandle,
) -> Result<HermesServingStatus, String> {
    let (config, client) = open(&app).await?;
    client
        .get::<HermesServingStatus>("/serving", config.request_timeout)
        .await
}

/// `PUT /serving/{id}` — sélectionne le dataset côté serveur et démarre ORS
/// lorsqu'il est exploitable. La réponse n'est pas interprétée : le store
/// relit `GET /serving`.
#[tauri::command]
pub async fn hermes_set_serving(
    app: tauri::AppHandle,
    dataset_id: String,
) -> Result<(), String> {
    validate_id(&dataset_id, "dataset")?;
    let (config, client) = open(&app).await?;
    client
        .put_no_body(&format!("/serving/{}", dataset_id), config.request_timeout)
        .await
}

/// `DELETE /serving` — arrête ORS et efface la sélection.
#[tauri::command]
pub async fn hermes_clear_serving(app: tauri::AppHandle) -> Result<(), String> {
    let (config, client) = open(&app).await?;
    client.delete("/serving", config.request_timeout).await
}

/// `POST /datasets` — crée un dataset composé uniquement de polygones (zones).
///
/// Pour des raisons de performance, l'IHM ne permet plus de créer un dataset à
/// partir de régions. La commande refuse donc une sélection vide ou un nombre
/// de zones supérieur au paramètre `Hermes.Limites.maxPolygonesParDataset`.
/// Les autres limites du serveur (surface totale, présence des PBF) restent
/// l'autorité et sont restituées telles quelles.
#[tauri::command]
pub async fn hermes_create_dataset(
    app: tauri::AppHandle,
    name: Option<String>,
    zones: Vec<String>,
) -> Result<HermesIdStatus, String> {
    let zones = normalize_ids(Some(zones), "zone")?.unwrap_or_default();
    if zones.is_empty() {
        return Err(coded(
            codes::INVALID,
            "Fournissez au moins un polygone (zone) pour créer un dataset.",
        ));
    }

    let name = name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let (config, client) = open(&app).await?;
    if zones.len() as u64 > config.max_polygons_per_dataset {
        return Err(coded(
            codes::INVALID,
            format!(
                "Un dataset est limité à {} polygones (paramètre Hermes.Limites.maxPolygonesParDataset).",
                config.max_polygons_per_dataset
            ),
        ));
    }

    // Précontrôle **best-effort** de la surface : une lecture `/polygons` en
    // échec (500, route indisponible) ne doit pas bloquer la création, le
    // serveur restant l'autorité. L'IHM applique déjà la même limite.
    if let Ok(polygons) = client
        .get::<Vec<HermesPolygon>>("/polygons", config.request_timeout)
        .await
    {
        if let Ok(dataset_area_km2) = dataset_surface_km2(&zones, &polygons) {
            if dataset_area_km2 > config.max_dataset_area_km2 {
                return Err(coded(
                    codes::INVALID,
                    format!(
                        "Surface du dataset {:.1} km² supérieure au maximum autorisé ({:.0} km², paramètre Hermes.Limites.surfaceMaxDatasetKm2).",
                        dataset_area_km2, config.max_dataset_area_km2
                    ),
                ));
            }
        }
    }

    client
        .post::<HermesIdStatus, _>(
            "/datasets",
            &HermesCreateDatasetRequest {
                name,
                regions: None,
                zones: Some(zones),
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
/// `GET /datasets/{id}/aux`. Le contrat v1.9 n'accepte que `force` : `skipMm`
/// a disparu du payload.
#[tauri::command]
pub async fn hermes_prepare_aux(
    app: tauri::AppHandle,
    dataset_id: String,
    force: bool,
) -> Result<HermesAuxPrepareResponse, String> {
    validate_id(&dataset_id, "dataset")?;
    let (config, client) = open(&app).await?;
    let body = serde_json::json!({ "force": force });
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
/// par l'IHM ; `index` est réservé par le serveur. La couleur n'est plus
/// traitée par l'application : le champ est envoyé à `null`.
#[tauri::command]
pub async fn hermes_create_polygon(
    app: tauri::AppHandle,
    id: String,
    name: String,
    description: Option<String>,
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
    validate_polygon_area(&geometry, config.max_polygon_area_km2)?;

    client
        .post::<HermesPolygon, _>(
            "/polygons",
            &HermesPolygonInput {
                id: Some(id),
                name,
                description: clean_optional(description),
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
    validate_polygon_area(&geometry, config.max_polygon_area_km2)?;

    client
        .put::<HermesPolygon, _>(
            &format!("/polygons/{}", polygon_id),
            &HermesPolygonInput {
                id: None,
                name,
                description: clean_optional(description),
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

/// Surface de l'anneau extérieur d'un polygone GeoJSON, en km².
///
/// L'anneau est refermé s'il ne l'est pas (le serveur le fait silencieusement)
/// et sa surface est calculée avec la même formule sphérique que les communes.
/// Une géométrie illisible est refusée par un code `HERMES_INVALID`.
pub(crate) fn polygon_outer_area_km2(geometry: &Value) -> Result<f64, String> {
    let coordinates = geometry
        .get("coordinates")
        .and_then(Value::as_array)
        .ok_or_else(|| coded(codes::INVALID, "Coordonnées du polygone absentes."))?;
    let outer = coordinates
        .first()
        .and_then(Value::as_array)
        .ok_or_else(|| coded(codes::INVALID, "Anneau extérieur du polygone absent."))?;

    let mut ring: Vec<[f64; 2]> = Vec::with_capacity(outer.len());
    for point in outer {
        let pair = point
            .as_array()
            .ok_or_else(|| coded(codes::INVALID, "Sommet de polygone illisible."))?;
        let lon = pair
            .first()
            .and_then(Value::as_f64)
            .ok_or_else(|| coded(codes::INVALID, "Longitude de polygone invalide."))?;
        let lat = pair
            .get(1)
            .and_then(Value::as_f64)
            .ok_or_else(|| coded(codes::INVALID, "Latitude de polygone invalide."))?;
        if !(-180.0..=180.0).contains(&lon) || !(-90.0..=90.0).contains(&lat) {
            return Err(coded(
                codes::INVALID,
                "Coordonnées hors WGS84 (lon ±180, lat ±90).",
            ));
        }
        ring.push([lon, lat]);
    }

    if ring.len() < 4 {
        return Err(coded(
            codes::INVALID,
            "Un polygone doit avoir au moins 4 sommets (anneau fermé).",
        ));
    }
    let first = ring[0];
    let last = ring[ring.len() - 1];
    if first != last {
        ring.push(first);
    }

    Ok(crate::gpx_communes::geometry::ring_area_m2(&ring) / 1_000_000.0)
}

/// Refuse un polygone dont la surface dépasse la limite configurée (km²).
fn validate_polygon_area(geometry: &Value, max_area_km2: f64) -> Result<(), String> {
    let area_km2 = polygon_outer_area_km2(geometry)?;
    if area_km2 > max_area_km2 {
        return Err(coded(
            codes::INVALID,
            format!(
                "Surface du polygone {:.1} km² supérieure au maximum autorisé ({:.0} km²).",
                area_km2, max_area_km2
            ),
        ));
    }
    Ok(())
}

/// Surface cumulée des zones d'un dataset, en km².
///
/// Les identifiants absents de `polygons` sont ignorés : le serveur refusera
/// de toute façon une zone inconnue. Une géométrie illisible en revanche est
/// signalée, pour ne pas laisser passer un dataset sous-évalué.
pub(crate) fn dataset_surface_km2(
    zones: &[String],
    polygons: &[HermesPolygon],
) -> Result<f64, String> {
    let mut total = 0.0;
    for zone_id in zones {
        if let Some(polygon) = polygons.iter().find(|candidate| &candidate.id == zone_id) {
            total += polygon_outer_area_km2(&polygon.geometry)?;
        }
    }
    Ok(total)
}
