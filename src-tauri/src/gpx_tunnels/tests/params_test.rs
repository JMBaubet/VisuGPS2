//! Tests de lecture des paramètres `Tunnels.*`.

use crate::gpx_tunnels::params::{
    default_tunnel_params, default_tunnel_support, params_from_settings, support_from_settings,
};
use crate::gpx_tunnels::types::{TunnelParams, TunnelSupport};

#[test]
fn defaults_match_the_prototype() {
    let params = default_tunnel_params();
    assert_eq!(params, TunnelParams::default());
    assert_eq!(params.distance_tolerance_m, 20.0);
    assert_eq!(params.angle_tolerance_deg, 25.0);
    assert_eq!(params.bbox_margin_m, 500.0);
    assert_eq!(params.bearing_window_m, 30.0);
    assert_eq!(params.min_run_length, 2);
    assert_eq!(params.min_length_m, 25.0);

    let support = default_tunnel_support();
    assert!(support.actif);
    assert_eq!(support.taille_max_pbf_mb, 500);
    assert_eq!(support.pbf_directory, None);
}

fn default_toml() -> toml::Table {
    toml::from_str(
        r#"
        [Osm.Fichiers]
        dossierPbf = ""
        tailleMaxPbf = 500

        [Tunnels.Detection]
        distanceTolerance = 20
        angleTolerance = 25
        bboxMargin = 500
        bearingWindow = 30
        minRunLength = 2
        minLength = 25

        [Tunnels.Support]
        actif = true
        "#,
    )
    .expect("TOML de défauts")
}

#[test]
fn user_overrides_take_precedence() {
    let overrides: toml::Table = toml::from_str(
        r#"
        [Osm.Fichiers]
        tailleMaxPbf = 1200
        dossierPbf = "/Volumes/USB/osm-data"

        [Tunnels.Detection]
        distanceTolerance = 35
        angleTolerance = 40
        bboxMargin = 1200
        bearingWindow = 50
        minRunLength = 3
        minLength = 40

        [Tunnels.Support]
        actif = false
        "#,
    )
    .expect("TOML de surcharges");

    let (params, support) = (
        params_from_settings(&default_toml(), &overrides),
        support_from_settings(&default_toml(), &overrides),
    );

    assert_eq!(params.distance_tolerance_m, 35.0);
    assert_eq!(params.angle_tolerance_deg, 40.0);
    assert_eq!(params.bbox_margin_m, 1200.0);
    assert_eq!(params.bearing_window_m, 50.0);
    assert_eq!(params.min_run_length, 3);
    assert_eq!(params.min_length_m, 40.0);
    assert!(!support.actif);
    assert_eq!(support.taille_max_pbf_mb, 1200);
    assert_eq!(
        support.pbf_directory.as_deref(),
        Some("/Volumes/USB/osm-data")
    );
}

#[test]
fn unoverridden_values_fall_back_to_the_schema() {
    let defaults: toml::Table = toml::from_str(
        r#"
        [Tunnels.Detection]
        distanceTolerance = 30

        [Tunnels.Support]
        actif = false
        "#,
    )
    .expect("TOML partiel");
    let overrides = toml::Table::new();

    let params = params_from_settings(&defaults, &overrides);
    let support = support_from_settings(&defaults, &overrides);

    assert_eq!(params.distance_tolerance_m, 30.0);
    assert_eq!(params.angle_tolerance_deg, 25.0);
    assert_eq!(params.min_run_length, 2);
    assert_eq!(params.min_length_m, 25.0);
    assert!(!support.actif);
    assert_eq!(support.pbf_directory, None);
}

#[test]
fn missing_paths_fall_back_to_defaults() {
    let empty = toml::Table::new();
    let params = params_from_settings(&empty, &empty);
    let support = support_from_settings(&empty, &empty);

    assert_eq!(params, TunnelParams::default());
    assert_eq!(support, TunnelSupport::default());
}

#[test]
fn blank_directory_is_none() {
    let overrides: toml::Table = toml::from_str(
        r#"
        [Osm.Fichiers]
        dossierPbf = "   "
        "#,
    )
    .expect("TOML de surcharges");

    let support = support_from_settings(&toml::Table::new(), &overrides);
    assert_eq!(support.pbf_directory, None);
}

#[test]
fn legacy_support_keys_are_read_when_the_new_keys_are_absent() {
    let overrides: toml::Table = toml::from_str(
        r#"
        [Tunnels.Support]
        tailleMaxPbf = 1200
        dossierPbf = "/Volumes/USB/osm-data"
        "#,
    )
    .expect("TOML de surcharges historiques");

    let support = support_from_settings(&toml::Table::new(), &overrides);
    assert_eq!(support.taille_max_pbf_mb, 1200);
    assert_eq!(
        support.pbf_directory.as_deref(),
        Some("/Volumes/USB/osm-data")
    );
}

#[test]
fn new_support_keys_take_precedence_over_legacy_keys() {
    let overrides: toml::Table = toml::from_str(
        r#"
        [Osm.Fichiers]
        tailleMaxPbf = 300
        dossierPbf = "/Volumes/NEW/osm-data"

        [Tunnels.Support]
        tailleMaxPbf = 1200
        dossierPbf = "/Volumes/OLD/osm-data"
        "#,
    )
    .expect("TOML de surcharges mixtes");

    let support = support_from_settings(&toml::Table::new(), &overrides);
    assert_eq!(support.taille_max_pbf_mb, 300);
    assert_eq!(
        support.pbf_directory.as_deref(),
        Some("/Volumes/NEW/osm-data")
    );
}

#[test]
fn legacy_tunnel_params_default_to_25_m() {
    let json = r#"{
        "distanceToleranceM": 20.0,
        "angleToleranceDeg": 25.0,
        "bboxMarginM": 500.0,
        "bearingWindowM": 30.0,
        "minRunLength": 2
    }"#;

    let params: TunnelParams =
        serde_json::from_str(json).expect("ancien fichier sans minLengthM");

    assert_eq!(params.min_length_m, 25.0);
}

#[test]
fn min_run_length_is_at_least_one() {
    let overrides: toml::Table = toml::from_str(
        r#"
        [Tunnels.Detection]
        minRunLength = 0
        "#,
    )
    .expect("TOML de surcharges");

    let params = params_from_settings(&toml::Table::new(), &overrides);
    assert_eq!(params.min_run_length, 1);
}
