//! Lecture des paramètres OSM partagés.
//!
//! Le dossier et le plafond vivent dans `Osm.Fichiers.*` ; le lecteur
//! `gpx_tunnels::params::support_from_settings` applique déjà la migration des
//! anciennes clés `Tunnels.Support.*`. Les durées de validité sont propres à la
//! gestion des fichiers.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::Manager;
use tokio::sync::RwLock;

use crate::settings::{get_toml_value_by_path, SettingsState};

/// Paramètres nécessaires à l'inventaire et aux commandes OSM.
#[derive(Debug, Clone, PartialEq)]
pub struct OsmSettings {
    pub pbf_directory: Option<PathBuf>,
    pub max_pbf_mb: u64,
    pub max_validity_months: u32,
    pub renewal_weeks: u32,
}

impl Default for OsmSettings {
    fn default() -> Self {
        Self {
            pbf_directory: None,
            max_pbf_mb: 500,
            max_validity_months: 12,
            renewal_weeks: 8,
        }
    }
}

/// Résout les paramètres OSM depuis les tables TOML : surcharges utilisateur
/// prioritaires, puis schéma par défaut. Les durées sont bornées aux valeurs
/// déclarées dans `settings.default.toml` (6–36 mois, 4–52 semaines). Pure et
/// testable.
pub fn settings_from_toml(
    default_toml: &toml::Table,
    user_overrides: &toml::Table,
) -> OsmSettings {
    let support =
        crate::gpx_tunnels::params::support_from_settings(default_toml, user_overrides);
    let fallback = OsmSettings::default();

    let integer = |path: &str, default: u32| -> u32 {
        match get_toml_value_by_path(user_overrides, path)
            .or_else(|| get_toml_value_by_path(default_toml, path))
        {
            Some(toml::Value::Integer(value)) => {
                u32::try_from((*value).max(0)).unwrap_or(u32::MAX)
            }
            Some(toml::Value::Float(value)) => {
                value.max(0.0).round().min(u32::MAX as f64) as u32
            }
            _ => default,
        }
    };

    OsmSettings {
        pbf_directory: support.pbf_directory.map(PathBuf::from),
        max_pbf_mb: support.taille_max_pbf_mb,
        max_validity_months: integer(
            "Osm.Validite.dureeMaxMois",
            fallback.max_validity_months,
        )
        .clamp(6, 36),
        renewal_weeks: integer(
            "Osm.Validite.renouvellementSemaines",
            fallback.renewal_weeks,
        )
        .clamp(4, 52),
    }
}

/// Lit les paramètres OSM depuis l'état de configuration managé. Ne panique
/// jamais : en l'absence d'état (tests, démarrage incomplet), les défauts sont
/// utilisés.
pub async fn read_settings(app: &tauri::AppHandle) -> OsmSettings {
    let Some(state) = app.try_state::<Arc<RwLock<SettingsState>>>() else {
        return OsmSettings::default();
    };
    let state = state.inner().clone();
    let guard = state.read().await;
    settings_from_toml(&guard.default_toml, &guard.user_overrides)
}
