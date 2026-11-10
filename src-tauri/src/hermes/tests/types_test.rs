//! Tests de désérialisation des réponses `hermes` et de sérialisation IPC.
//!
//! Le serveur mélange `snake_case` (`/datasets`, `/regions`, `/polygons`) et
//! `camelCase` (`/datasets/{id}/aux`). Ces tests vérifient que les alias
//! acceptent les deux et que le frontend ne reçoit que du camelCase.

use serde_json::json;

use crate::hermes::types::{
    HermesAuxPrepareResponse, HermesAuxStatus, HermesDataset, HermesIdStatus, HermesPolygon,
    HermesPolygonInput, HermesRegion,
};

#[test]
fn dataset_accepts_snake_case_and_serializes_camel_case() {
    let raw = json!([{
        "id": "provence-alpes-cote-dazur",
        "regions": ["provence-alpes-cote-dazur"],
        "zones": null,
        "created_at": "2026-11-07T16:42:00Z",
        "status": "ready",
        "size_bytes": 1_750_000_000u64,
        "ors_version": "9.9.0",
        "stale": false
    }]);

    let datasets: Vec<HermesDataset> =
        serde_json::from_value(raw).expect("dataset snake_case lisible");
    let dataset = &datasets[0];
    assert_eq!(dataset.id, "provence-alpes-cote-dazur");
    assert_eq!(
        dataset.regions.as_deref(),
        Some(&["provence-alpes-cote-dazur".to_string()][..])
    );
    assert_eq!(dataset.zones, None);
    assert_eq!(dataset.created_at, "2026-11-07T16:42:00Z");
    assert_eq!(dataset.size_bytes, 1_750_000_000);
    assert_eq!(dataset.ors_version, "9.9.0");
    assert!(!dataset.stale);
    assert_eq!(dataset.last_log_tail, None);

    let out = serde_json::to_value(dataset).expect("dataset sérialisable");
    assert_eq!(out["createdAt"], "2026-11-07T16:42:00Z");
    assert_eq!(out["sizeBytes"], 1_750_000_000u64);
    assert_eq!(out["orsVersion"], "9.9.0");
}

#[test]
fn region_accepts_snake_case_and_serializes_camel_case() {
    let raw = json!({
        "id": "provence-alpes-cote-dazur",
        "bbox": [4.23, 42.98, 7.72, 45.13],
        "has_poly": true,
        "geojson": {"type": "Polygon", "coordinates": []},
        "geofabrik_id": "europe/france/provence-alpes-cote-d-azur",
        "updated_at": "2026-11-07T16:00:00Z"
    });

    let region: HermesRegion =
        serde_json::from_value(raw).expect("région snake_case lisible");
    assert_eq!(region.bbox, [4.23, 42.98, 7.72, 45.13]);
    assert!(region.has_poly);
    assert_eq!(
        region.geofabrik_id.as_deref(),
        Some("europe/france/provence-alpes-cote-d-azur")
    );
    assert_eq!(region.updated_at.as_deref(), Some("2026-11-07T16:00:00Z"));

    let out = serde_json::to_value(&region).expect("région sérialisable");
    assert_eq!(out["hasPoly"], true);
    assert_eq!(
        out["geofabrikId"],
        "europe/france/provence-alpes-cote-d-azur"
    );
    assert_eq!(out["updatedAt"], "2026-11-07T16:00:00Z");
}

#[test]
fn polygon_accepts_snake_case_and_serializes_camel_case() {
    let raw = json!({
        "id": "ma-zone-nice",
        "name": "Zone Nice",
        "description": null,
        "color": "#3b82f6",
        "geometry": {"type": "Polygon", "coordinates": []},
        "bbox": [7.26, 43.71, 7.28, 43.73],
        "created_at": "2026-11-06T10:30:00Z",
        "updated_at": "2026-11-06T10:30:00Z",
        "revision": 1
    });

    let polygon: HermesPolygon =
        serde_json::from_value(raw).expect("polygone snake_case lisible");
    assert_eq!(polygon.id, "ma-zone-nice");
    assert_eq!(polygon.description, None);
    assert_eq!(polygon.revision, 1);

    let out = serde_json::to_value(&polygon).expect("polygone sérialisable");
    assert_eq!(out["createdAt"], "2026-11-06T10:30:00Z");
    assert_eq!(out["updatedAt"], "2026-11-06T10:30:00Z");
}

#[test]
fn aux_status_accepts_camel_case() {
    let raw = json!({
        "datasetId": "provence-alpes-cote-dazur",
        "communes": {
            "name": "communes.geojson",
            "exists": true,
            "sizeBytes": 46_502_397u64,
            "modifiedAt": "2026-11-09T12:44:51Z"
        },
        "tunnels": {
            "name": "tunnels-logiques.json",
            "exists": true,
            "sizeBytes": 187_432u64,
            "modifiedAt": "2026-11-09T20:29:01Z"
        },
        "mm": {
            "name": "mm/",
            "exists": true,
            "sizeBytes": 2_402_864u64,
            "modifiedAt": "2026-11-09T20:40:45Z"
        },
        "preparing": false,
        "startedAt": null,
        "lastError": null,
        "loadedInMemory": false,
        "activeDataset": null
    });

    let aux: HermesAuxStatus =
        serde_json::from_value(raw).expect("aux camelCase lisible");
    assert_eq!(aux.dataset_id, "provence-alpes-cote-dazur");
    assert!(aux.communes.exists);
    assert_eq!(aux.communes.size_bytes, Some(46_502_397));
    assert_eq!(aux.tunnels.name, "tunnels-logiques.json");
    assert_eq!(aux.mm.size_bytes, Some(2_402_864));
    assert!(!aux.preparing);
    assert!(!aux.loaded_in_memory);

    let out = serde_json::to_value(&aux).expect("aux sérialisable");
    assert_eq!(out["datasetId"], "provence-alpes-cote-dazur");
    assert_eq!(out["loadedInMemory"], false);
}

#[test]
fn aux_prepare_response_accepts_camel_case_and_serializes_camel_case() {
    let raw = json!({
        "datasetId": "test-monaco-ds",
        "status": "preparing",
        "startedAt": "2026-11-09T20:40:44Z",
        "message": "Préparation en tâche de fond. Consulter GET /datasets/{id}/aux"
    });

    let response: HermesAuxPrepareResponse =
        serde_json::from_value(raw).expect("réponse aux/prepare lisible");
    assert_eq!(response.dataset_id, "test-monaco-ds");
    assert_eq!(response.status, "preparing");
    assert_eq!(response.started_at, "2026-11-09T20:40:44Z");
    assert!(response.message.is_some());

    let out = serde_json::to_value(&response).expect("réponse aux/prepare sérialisable");
    assert_eq!(out["datasetId"], "test-monaco-ds");
    assert_eq!(out["startedAt"], "2026-11-09T20:40:44Z");
    assert_eq!(out["status"], "preparing");
}

#[test]
fn polygon_input_omits_optional_fields() {
    let input = HermesPolygonInput {
        id: None,
        name: "Zone Nice".to_string(),
        description: None,
        color: Some("#3b82f6".to_string()),
        geometry: json!({"type": "Polygon", "coordinates": []}),
    };

    let out = serde_json::to_value(&input).expect("payload sérialisable");
    assert!(out.get("id").is_none());
    assert!(out.get("description").is_none());
    assert_eq!(out["name"], "Zone Nice");
    assert_eq!(out["color"], "#3b82f6");
}

#[test]
fn id_status_is_a_plain_contract() {
    let value = json!({"id": "nice-monaco", "status": "preparing"});
    let status: HermesIdStatus = serde_json::from_value(value).expect("statut lisible");
    assert_eq!(status.id, "nice-monaco");
    assert_eq!(status.status, "preparing");
}
