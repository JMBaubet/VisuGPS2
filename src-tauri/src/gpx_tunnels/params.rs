//! Lecture des paramètres `Tunnels.*`.
//!
//! Même découpage que `gpx_altimetrie::params` : une fonction Tauri
//! (`read_config`) qui ne panique jamais, et des fonctions pures
//! (`params_from_settings`, `support_from_settings`) directement testables.
//!
//! Contrairement aux lecteurs historiques qui demandent un
//! `std::sync::RwLock` alors que l'état est un `tokio::sync::RwLock`, ce
//! module utilise le **vrai** type managé : les surcharges utilisateur sont
//! donc réellement prises en compte.

use std::sync::Arc;

use tauri::Manager;
use tokio::sync::RwLock;

use crate::settings::{get_toml_value_by_path, SettingsState};

use super::types::{TunnelParams, TunnelSupport};

/// Lit les paramètres et le support OSM (`Tunnels.*`) avec repli sur les
/// défauts. Ne panique jamais : l'appelant est la file de préparation
/// automatique.
pub async fn read_config(app: &tauri::AppHandle) -> (TunnelParams, TunnelSupport) {
    let Some(state) = app.try_state::<Arc<RwLock<SettingsState>>>() else {
        return (TunnelParams::default(), TunnelSupport::default());
    };
    // Cloner l'Arc évite de garder l'emprunt de `State` pendant l'await.
    let state = state.inner().clone();
    let guard = state.read().await;
    (
        params_from_settings(&guard.default_toml, &guard.user_overrides),
        support_from_settings(&guard.default_toml, &guard.user_overrides),
    )
}

/// Valeurs par défaut du pipeline — miroir de la section `Tunnels.Detection.*`
/// de `settings.default.toml` et du prototype validé.
pub fn default_tunnel_params() -> TunnelParams {
    TunnelParams::default()
}

/// Valeurs par défaut du support OSM.
pub fn default_tunnel_support() -> TunnelSupport {
    TunnelSupport::default()
}

/// Résout les paramètres de détection depuis les tables TOML : surcharge
/// utilisateur prioritaire, puis schéma par défaut, puis repli. Pure et
/// testable.
pub fn params_from_settings(
    default_toml: &toml::Table,
    user_overrides: &toml::Table,
) -> TunnelParams {
    let fallback = default_tunnel_params();

    let number = |path: &str, default: f64| -> f64 {
        let value = get_toml_value_by_path(user_overrides, path)
            .or_else(|| get_toml_value_by_path(default_toml, path));
        match value {
            Some(toml::Value::Float(v)) => *v,
            Some(toml::Value::Integer(v)) => *v as f64,
            _ => default,
        }
    };

    let integer = |path: &str, default: usize| -> usize {
        number(path, default as f64).round().max(1.0) as usize
    };

    TunnelParams {
        distance_tolerance_m: number(
            "Tunnels.Detection.distanceTolerance",
            fallback.distance_tolerance_m,
        ),
        angle_tolerance_deg: number(
            "Tunnels.Detection.angleTolerance",
            fallback.angle_tolerance_deg,
        ),
        bbox_margin_m: number("Tunnels.Detection.bboxMargin", fallback.bbox_margin_m),
        bearing_window_m: number(
            "Tunnels.Detection.bearingWindow",
            fallback.bearing_window_m,
        ),
        min_run_length: integer(
            "Tunnels.Detection.minRunLength",
            fallback.min_run_length,
        ),
    }
}

/// Résout le support OSM depuis les tables TOML. Pure et testable.
pub fn support_from_settings(
    default_toml: &toml::Table,
    user_overrides: &toml::Table,
) -> TunnelSupport {
    let fallback = default_tunnel_support();

    let active = get_toml_value_by_path(user_overrides, "Tunnels.Support.actif")
        .or_else(|| get_toml_value_by_path(default_toml, "Tunnels.Support.actif"))
        .and_then(|value| value.as_bool())
        .unwrap_or(fallback.actif);

    let directory = get_toml_value_by_path(user_overrides, "Tunnels.Support.dossierPbf")
        .or_else(|| get_toml_value_by_path(default_toml, "Tunnels.Support.dossierPbf"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    TunnelSupport {
        actif: active,
        pbf_directory: directory,
    }
}
