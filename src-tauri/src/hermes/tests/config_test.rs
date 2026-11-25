//! Tests de la lecture des paramètres `Hermes.*`.

use crate::hermes::config::{
    config_from_tables, normalize_url, DEFAULT_BASE_URL, DEFAULT_MAX_DATASET_AREA_KM2,
    DEFAULT_MAX_POLYGON_AREA_KM2, DEFAULT_MAX_POLYGONS_PER_DATASET, DEFAULT_ORS_BASE_URL,
    DEFAULT_WAKE_ADDRESS, DEFAULT_WAKE_MAC,
};

fn table(text: &str) -> toml::Table {
    toml::from_str(text).expect("TOML de test valide")
}

#[test]
fn defaults_are_used_without_overrides() {
    let empty = toml::Table::new();
    let config = config_from_tables(&empty, &empty);

    assert_eq!(config.base_url, DEFAULT_BASE_URL);
    assert_eq!(config.ors_base_url, DEFAULT_ORS_BASE_URL);
    assert_eq!(config.request_timeout.as_secs(), 30);
    assert_eq!(config.keep_alive.map(|period| period.as_secs()), Some(30));
    assert_eq!(config.wake_address, DEFAULT_WAKE_ADDRESS);
    assert_eq!(config.wake_mac, DEFAULT_WAKE_MAC);
    assert_eq!(config.wake_port, 9);
    assert_eq!(config.wake_timeout.as_secs(), 120);
    assert_eq!(
        config.max_polygons_per_dataset,
        DEFAULT_MAX_POLYGONS_PER_DATASET
    );
    assert_eq!(config.max_polygon_area_km2, DEFAULT_MAX_POLYGON_AREA_KM2);
    assert_eq!(config.max_dataset_area_km2, DEFAULT_MAX_DATASET_AREA_KM2);
}

#[test]
fn overrides_are_applied_and_urls_are_normalized() {
    let overrides = table(
        r#"
[Hermes.Connexion]
baseUrl = "http://192.168.1.10:9000/api/v1/"
orsBaseUrl = "https://hermes.local/ors/v2/"
delaiSec = 10
keepAliveSec = 0

[Hermes.Reveil]
adresse = "192.168.1.255"
mac = "AA-BB-CC-DD-EE-FF"
port = 7
delaiMaxSec = 60

[Hermes.Limites]
maxPolygonesParDataset = 5
surfaceMaxPolygoneKm2 = 25000
surfaceMaxDatasetKm2 = 35000
"#,
    );
    let empty = toml::Table::new();
    let config = config_from_tables(&empty, &overrides);

    assert_eq!(config.base_url, "http://192.168.1.10:9000/api/v1");
    assert_eq!(config.ors_base_url, "https://hermes.local/ors/v2");
    assert_eq!(config.request_timeout.as_secs(), 10);
    assert_eq!(config.keep_alive, None);
    assert_eq!(config.wake_address, "192.168.1.255");
    assert_eq!(config.wake_mac, "AA-BB-CC-DD-EE-FF");
    assert_eq!(config.wake_port, 7);
    assert_eq!(config.wake_timeout.as_secs(), 60);
    assert_eq!(config.max_polygons_per_dataset, 5);
    assert_eq!(config.max_polygon_area_km2, 25_000.0);
    assert_eq!(config.max_dataset_area_km2, 35_000.0);
}

#[test]
fn invalid_urls_fall_back_to_defaults() {
    let overrides = table(
        r#"
[Hermes.Connexion]
baseUrl = "ftp://invalide"
orsBaseUrl = "   "
"#,
    );
    let empty = toml::Table::new();
    let config = config_from_tables(&empty, &overrides);

    assert_eq!(config.base_url, DEFAULT_BASE_URL);
    assert_eq!(config.ors_base_url, DEFAULT_ORS_BASE_URL);
}

#[test]
fn numeric_values_are_clamped() {
    let overrides = table(
        r#"
[Hermes.Connexion]
delaiSec = 1
keepAliveSec = 999

[Hermes.Reveil]
port = 0
delaiMaxSec = 9999

[Hermes.Limites]
maxPolygonesParDataset = 0
surfaceMaxPolygoneKm2 = 10
surfaceMaxDatasetKm2 = 10
"#,
    );
    let empty = toml::Table::new();
    let config = config_from_tables(&empty, &overrides);

    assert_eq!(config.request_timeout.as_secs(), 2);
    assert_eq!(config.keep_alive.map(|period| period.as_secs()), Some(300));
    assert_eq!(config.wake_port, 1);
    assert_eq!(config.wake_timeout.as_secs(), 300);
    assert_eq!(config.max_polygons_per_dataset, 1);
    assert_eq!(config.max_polygon_area_km2, 1_000.0);
    assert_eq!(config.max_dataset_area_km2, 1_000.0);
}

#[test]
fn numeric_upper_bounds_are_clamped() {
    let overrides = table(
        r#"
[Hermes.Connexion]
delaiSec = 999

[Hermes.Reveil]
port = 70000

[Hermes.Limites]
maxPolygonesParDataset = 99
surfaceMaxPolygoneKm2 = 999999
surfaceMaxDatasetKm2 = 999999
"#,
    );
    let empty = toml::Table::new();
    let config = config_from_tables(&empty, &overrides);

    assert_eq!(config.request_timeout.as_secs(), 60);
    assert_eq!(config.wake_port, 65535);
    assert_eq!(config.max_polygons_per_dataset, 10);
    assert_eq!(config.max_polygon_area_km2, 100_000.0);
    assert_eq!(config.max_dataset_area_km2, 100_000.0);
}

#[test]
fn normalize_url_accepts_http_and_https_only() {
    assert_eq!(
        normalize_url("  http://100.118.62.10:9000/api/v1/  "),
        Some("http://100.118.62.10:9000/api/v1".to_string())
    );
    assert_eq!(
        normalize_url("https://hermes.local/ors/v2///"),
        Some("https://hermes.local/ors/v2".to_string())
    );
    assert_eq!(normalize_url("ftp://hermes.local"), None);
    assert_eq!(normalize_url("   "), None);
}
