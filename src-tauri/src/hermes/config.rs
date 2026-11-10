//! Lecture des paramètres `Hermes.*` et construction de la configuration du
//! client.
//!
//! Même découpage que `gpx_tunnels::params` : une fonction Tauri
//! (`read_config`) qui ne panique jamais, et une fonction pure
//! (`config_from_tables`) directement testable.
//!
//! Les valeurs sont surchargées par mode d'exécution, comme tous les
//! paramètres : un mode EVAL peut pointer vers un serveur de test, un mode OPE
//! vers la production.

use std::sync::Arc;
use std::time::Duration;

use tauri::Manager;
use tokio::sync::RwLock;

use crate::settings::{get_toml_value_by_path, SettingsState};

pub(crate) const DEFAULT_BASE_URL: &str = "http://10.15.58.131:9000/api/v1";
pub(crate) const DEFAULT_ORS_BASE_URL: &str = "http://10.15.58.131:8002/ors/v2";
pub(crate) const DEFAULT_WAKE_ADDRESS: &str = "87.91.31.205";
pub(crate) const DEFAULT_WAKE_MAC: &str = "68:5b:35:a8:5c:65";
pub(crate) const DEFAULT_WAKE_PORT: u16 = 9;
pub(crate) const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;
pub(crate) const DEFAULT_KEEP_ALIVE_SECS: u64 = 30;
pub(crate) const DEFAULT_WAKE_TIMEOUT_SECS: u64 = 120;

/// Configuration effective du client `hermes`.
#[derive(Debug, Clone, PartialEq)]
pub struct HermesConfig {
    /// Base de l'API contrôleur, sans barre oblique finale.
    pub base_url: String,
    /// Base du moteur ORS hébergé par hermes, sans barre oblique finale.
    pub ors_base_url: String,
    /// Délai par requête des routes courtes.
    pub request_timeout: Duration,
    /// Période du keep-alive ; `None` quand il est désactivé (0).
    pub keep_alive: Option<Duration>,
    pub wake_address: String,
    pub wake_mac: String,
    pub wake_port: u16,
    /// Attente maximale après un réveil WoL.
    pub wake_timeout: Duration,
    /// Dataset actif, ou `None` si aucun n'est sélectionné.
    pub active_dataset: Option<String>,
}

impl Default for HermesConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            ors_base_url: DEFAULT_ORS_BASE_URL.to_string(),
            request_timeout: Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
            keep_alive: Some(Duration::from_secs(DEFAULT_KEEP_ALIVE_SECS)),
            wake_address: DEFAULT_WAKE_ADDRESS.to_string(),
            wake_mac: DEFAULT_WAKE_MAC.to_string(),
            wake_port: DEFAULT_WAKE_PORT,
            wake_timeout: Duration::from_secs(DEFAULT_WAKE_TIMEOUT_SECS),
            active_dataset: None,
        }
    }
}

/// Lit la configuration depuis l'état des paramètres. Ne panique jamais : si
/// l'état n'est pas encore disponible, les défauts sont utilisés.
pub async fn read_config(app: &tauri::AppHandle) -> HermesConfig {
    let Some(state) = app.try_state::<Arc<RwLock<SettingsState>>>() else {
        return HermesConfig::default();
    };
    // Cloner l'Arc évite de garder l'emprunt de `State` pendant l'await.
    let state = state.inner().clone();
    let guard = state.read().await;
    config_from_tables(&guard.default_toml, &guard.user_overrides)
}

/// Résout la configuration depuis les tables TOML : surcharge utilisateur
/// prioritaire, puis schéma par défaut, puis repli. Pure et testable.
pub(crate) fn config_from_tables(
    default_toml: &toml::Table,
    user_overrides: &toml::Table,
) -> HermesConfig {
    let fallback = HermesConfig::default();

    let base_url = string_at(default_toml, user_overrides, "Hermes.Connexion.baseUrl")
        .as_deref()
        .and_then(normalize_url)
        .unwrap_or(fallback.base_url);

    let ors_base_url = string_at(default_toml, user_overrides, "Hermes.Connexion.orsBaseUrl")
        .as_deref()
        .and_then(normalize_url)
        .unwrap_or(fallback.ors_base_url);

    let request_timeout_secs = u64_at(
        default_toml,
        user_overrides,
        "Hermes.Connexion.delaiSec",
    )
    .unwrap_or(DEFAULT_REQUEST_TIMEOUT_SECS)
    .clamp(2, 60);

    let keep_alive_secs = u64_at(
        default_toml,
        user_overrides,
        "Hermes.Connexion.keepAliveSec",
    )
    .unwrap_or(DEFAULT_KEEP_ALIVE_SECS)
    .min(300);
    let keep_alive = if keep_alive_secs == 0 {
        None
    } else {
        Some(Duration::from_secs(keep_alive_secs))
    };

    let wake_address = string_at(default_toml, user_overrides, "Hermes.Reveil.adresse")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_WAKE_ADDRESS.to_string());

    let wake_mac = string_at(default_toml, user_overrides, "Hermes.Reveil.mac")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_WAKE_MAC.to_string());

    let wake_port = u64_at(default_toml, user_overrides, "Hermes.Reveil.port")
        .unwrap_or(DEFAULT_WAKE_PORT as u64)
        .clamp(1, 65_535) as u16;

    let wake_timeout_secs = u64_at(
        default_toml,
        user_overrides,
        "Hermes.Reveil.delaiMaxSec",
    )
    .unwrap_or(DEFAULT_WAKE_TIMEOUT_SECS)
    .clamp(30, 300);

    let active_dataset = string_at(default_toml, user_overrides, "Hermes.Dataset.actif")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    HermesConfig {
        base_url,
        ors_base_url,
        request_timeout: Duration::from_secs(request_timeout_secs),
        keep_alive,
        wake_address,
        wake_mac,
        wake_port,
        wake_timeout: Duration::from_secs(wake_timeout_secs),
        active_dataset,
    }
}

/// Normalise une URL : espaces retirés, barres obliques finales supprimées,
/// schéma `http`/`https` obligatoire. `None` si la valeur est inutilisable.
pub(crate) fn normalize_url(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn value_at<'a>(
    default_toml: &'a toml::Table,
    user_overrides: &'a toml::Table,
    path: &str,
) -> Option<&'a toml::Value> {
    get_toml_value_by_path(user_overrides, path)
        .or_else(|| get_toml_value_by_path(default_toml, path))
}

fn string_at(
    default_toml: &toml::Table,
    user_overrides: &toml::Table,
    path: &str,
) -> Option<String> {
    value_at(default_toml, user_overrides, path)
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn u64_at(default_toml: &toml::Table, user_overrides: &toml::Table, path: &str) -> Option<u64> {
    match value_at(default_toml, user_overrides, path) {
        Some(toml::Value::Integer(value)) => Some((*value).max(0) as u64),
        Some(toml::Value::Float(value)) => Some(value.max(0.0).round() as u64),
        _ => None,
    }
}
