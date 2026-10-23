//! Tests de lecture des paramètres `Osm.*`.

use std::path::PathBuf;

use crate::osm::params::{settings_from_toml, OsmSettings};

#[test]
fn defaults_match_the_settings_schema() {
    let settings = settings_from_toml(&toml::Table::new(), &toml::Table::new());
    assert_eq!(settings, OsmSettings::default());
    assert_eq!(settings.pbf_directory, None);
    assert_eq!(settings.max_pbf_mb, 500);
    assert_eq!(settings.max_validity_months, 12);
    assert_eq!(settings.renewal_weeks, 8);
}

#[test]
fn schema_values_are_used_when_the_user_has_not_overridden_them() {
    let defaults: toml::Table = toml::from_str(
        r#"
        [Osm.Fichiers]
        dossierPbf = "/schema/osm-data"
        tailleMaxPbf = 650

        [Osm.Validite]
        dureeMaxMois = 18
        renouvellementSemaines = 10
        "#,
    )
    .expect("TOML de schéma");

    let settings = settings_from_toml(&defaults, &toml::Table::new());
    assert_eq!(
        settings.pbf_directory,
        Some(PathBuf::from("/schema/osm-data"))
    );
    assert_eq!(settings.max_pbf_mb, 650);
    assert_eq!(settings.max_validity_months, 18);
    assert_eq!(settings.renewal_weeks, 10);
}

#[test]
fn overrides_take_precedence() {
    let overrides: toml::Table = toml::from_str(
        r#"
        [Osm.Fichiers]
        dossierPbf = "/Volumes/USB/osm-data"
        tailleMaxPbf = 800

        [Osm.Validite]
        dureeMaxMois = 24
        renouvellementSemaines = 12
        "#,
    )
    .expect("TOML de surcharges");

    let settings = settings_from_toml(&toml::Table::new(), &overrides);
    assert_eq!(
        settings.pbf_directory,
        Some(PathBuf::from("/Volumes/USB/osm-data"))
    );
    assert_eq!(settings.max_pbf_mb, 800);
    assert_eq!(settings.max_validity_months, 24);
    assert_eq!(settings.renewal_weeks, 12);
}

#[test]
fn durations_are_clamped_to_the_declared_bounds() {
    let overrides: toml::Table = toml::from_str(
        r#"
        [Osm.Validite]
        dureeMaxMois = 1
        renouvellementSemaines = 100
        "#,
    )
    .expect("TOML hors bornes");

    let settings = settings_from_toml(&toml::Table::new(), &overrides);
    assert_eq!(settings.max_validity_months, 6);
    assert_eq!(settings.renewal_weeks, 52);

    let overrides: toml::Table = toml::from_str(
        r#"
        [Osm.Validite]
        dureeMaxMois = 100
        renouvellementSemaines = 1
        "#,
    )
    .expect("TOML hors bornes");

    let settings = settings_from_toml(&toml::Table::new(), &overrides);
    assert_eq!(settings.max_validity_months, 36);
    assert_eq!(settings.renewal_weeks, 4);
}

#[test]
fn legacy_support_keys_are_read_as_a_fallback() {
    let overrides: toml::Table = toml::from_str(
        r#"
        [Tunnels.Support]
        dossierPbf = "/Volumes/OLD/osm-data"
        tailleMaxPbf = 1200
        "#,
    )
    .expect("TOML historique");

    let settings = settings_from_toml(&toml::Table::new(), &overrides);
    assert_eq!(
        settings.pbf_directory,
        Some(PathBuf::from("/Volumes/OLD/osm-data"))
    );
    assert_eq!(settings.max_pbf_mb, 1200);
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

    let settings = settings_from_toml(&toml::Table::new(), &overrides);
    assert_eq!(settings.pbf_directory, None);
}
