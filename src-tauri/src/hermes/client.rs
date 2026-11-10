//! Client HTTP du contrôleur `hermes`.
//!
//! Le réseau vit côté Rust, pas dans la WebView : `hermes-api` n'expose pas de
//! middleware CORS, et le WoL exige un socket UDP. `reqwest` est déjà une
//! dépendance de l'application (téléchargements Geofabrik) ; la feature `json`
//! n'est pas activée, le corps est donc sérialisé avec `serde_json` et envoyé
//! comme `String`.
//!
//! Politique de reprise (contrat v1.10) : trois tentatives avec backoff
//! exponentiel (1 s, 2 s) sur les erreurs réseau et les 5xx des requêtes
//! **idempotentes** (`GET`, `DELETE`). Les `POST` ne sont jamais rejoués :
//! `POST /datasets` et `POST /communes/detect` créent ou relancent une
//! opération, un rejeu produirait un doublon.

use std::sync::OnceLock;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use super::config::HermesConfig;
use super::types::{coded, codes};

/// Client HTTP partagé : son pool de connexions est réutilisé entre les
/// commandes (le polling de dataset peut durer 45 min). Le délai est porté par
/// chaque requête, pas par le client.
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn http_client() -> Result<reqwest::Client, String> {
    if let Some(client) = HTTP_CLIENT.get() {
        return Ok(client.clone());
    }
    let client = reqwest::Client::builder()
        .user_agent(format!("VisuGPS2/{}", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .map_err(|error| {
            coded(
                codes::SERVER,
                format!("Création du client HTTP impossible : {}", error),
            )
        })?;
    // `set` échoue seulement si un autre thread a gagné la course : le client
    // déjà installé est équivalent, on l'utilise.
    let _ = HTTP_CLIENT.set(client.clone());
    Ok(client)
}

/// Client HTTP et URLs de base.
pub struct HermesClient {
    base_url: String,
    ors_base_url: String,
    http: reqwest::Client,
}

impl HermesClient {
    /// Construit le client. Le pool de connexions est partagé ; chaque requête
    /// porte son propre délai via `RequestBuilder::timeout`.
    pub fn new(config: &HermesConfig) -> Result<Self, String> {
        Ok(Self {
            base_url: config.base_url.clone(),
            ors_base_url: config.ors_base_url.clone(),
            http: http_client()?,
        })
    }

    /// `GET` idempotent avec reprise.
    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        timeout: Duration,
    ) -> Result<T, String> {
        let url = join_url(&self.base_url, path);
        self.send_json(reqwest::Method::GET, &url, None, timeout, true)
            .await
    }

    /// `GET` sans reprise, pour un ping dont l'échec doit être immédiat.
    pub async fn get_once<T: DeserializeOwned>(
        &self,
        path: &str,
        timeout: Duration,
    ) -> Result<T, String> {
        let url = join_url(&self.base_url, path);
        self.send_json(reqwest::Method::GET, &url, None, timeout, false)
            .await
    }

    /// `GET` sur une URL absolue (santé ORS), sans reprise.
    pub async fn get_absolute_once<T: DeserializeOwned>(
        &self,
        url: &str,
        timeout: Duration,
    ) -> Result<T, String> {
        self.send_json(reqwest::Method::GET, url, None, timeout, false)
            .await
    }

    /// `POST` sans reprise (non idempotent par convention).
    pub async fn post<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
        timeout: Duration,
    ) -> Result<T, String> {
        let url = join_url(&self.base_url, path);
        let body = serialize_body(body)?;
        self.send_json(reqwest::Method::POST, &url, Some(body), timeout, false)
            .await
    }

    /// `PUT` sans reprise (idempotent côté serveur, mais l'IHM gère la relance).
    pub async fn put<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
        timeout: Duration,
    ) -> Result<T, String> {
        let url = join_url(&self.base_url, path);
        let body = serialize_body(body)?;
        self.send_json(reqwest::Method::PUT, &url, Some(body), timeout, false)
            .await
    }

    /// `DELETE` idempotent avec reprise.
    pub async fn delete(&self, path: &str, timeout: Duration) -> Result<(), String> {
        let url = join_url(&self.base_url, path);
        let _response = self
            .execute(reqwest::Method::DELETE, &url, None, timeout, true)
            .await?;
        Ok(())
    }

    /// URL de santé ORS (`{orsBaseUrl}/health`).
    pub fn ors_health_url(&self) -> String {
        format!("{}/health", self.ors_base_url)
    }

    async fn send_json<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<String>,
        timeout: Duration,
        retry: bool,
    ) -> Result<T, String> {
        let response = self.execute(method, url, body, timeout, retry).await?;
        let text = response.text().await.map_err(|error| {
            coded(
                codes::SERVER,
                format!("Lecture de la réponse hermes impossible : {}", error),
            )
        })?;
        serde_json::from_str(&text).map_err(|error| {
            coded(
                codes::SERVER,
                format!("Réponse hermes illisible : {}", error),
            )
        })
    }

    async fn execute(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<String>,
        timeout: Duration,
        retry: bool,
    ) -> Result<reqwest::Response, String> {
        let attempts = if retry { 3 } else { 1 };
        let mut last_error = String::new();

        for attempt in 0..attempts {
            let mut request = self.http.request(method.clone(), url).timeout(timeout);
            if let Some(body) = body.clone() {
                request = request
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .body(body);
            }

            match request.send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(response);
                    }
                    let status = response.status();
                    let text = response.text().await.unwrap_or_default();
                    let error = coded(map_status(status.as_u16()), error_message(status, &text));
                    if retry && status.is_server_error() && attempt + 1 < attempts {
                        last_error = error;
                        tokio::time::sleep(backoff(attempt)).await;
                        continue;
                    }
                    return Err(error);
                }
                Err(reqwest_error) => {
                    let retryable = is_retryable(&reqwest_error);
                    let (code, message) = map_reqwest_error(&reqwest_error);
                    let error = coded(code, message);
                    if retry && retryable && attempt + 1 < attempts {
                        last_error = error;
                        tokio::time::sleep(backoff(attempt)).await;
                        continue;
                    }
                    return Err(error);
                }
            }
        }

        Err(last_error)
    }
}

fn serialize_body<B: Serialize + ?Sized>(body: &B) -> Result<String, String> {
    serde_json::to_string(body).map_err(|error| {
        coded(
            codes::SERVER,
            format!("Sérialisation de la requête hermes impossible : {}", error),
        )
    })
}

fn backoff(attempt: usize) -> Duration {
    Duration::from_secs(1u64 << attempt.min(2))
}

/// Une requête réseau est rejouable uniquement si l'échec est transitoire.
fn is_retryable(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect()
}

pub(crate) fn map_status(status: u16) -> &'static str {
    match status {
        400 | 405 | 422 => codes::INVALID,
        401 | 403 => codes::FORBIDDEN,
        404 => codes::NOT_FOUND,
        409 => codes::CONFLICT,
        500..=599 => codes::SERVER,
        _ => codes::SERVER,
    }
}

fn map_reqwest_error(error: &reqwest::Error) -> (&'static str, String) {
    if error.is_timeout() {
        (
            codes::TIMEOUT,
            format!("Délai dépassé vers hermes : {}", error),
        )
    } else if error.is_connect() {
        (
            codes::OFFLINE,
            format!("Connexion à hermes impossible : {}", error),
        )
    } else {
        (
            codes::SERVER,
            format!("Requête hermes impossible : {}", error),
        )
    }
}

fn error_message(status: reqwest::StatusCode, text: &str) -> String {
    if let Some(detail) = parse_detail(text) {
        return detail;
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        format!("HTTP {}", status.as_u16())
    } else {
        format!("HTTP {} : {}", status.as_u16(), truncate(trimmed, 300))
    }
}

/// Extrait le message d'erreur FastAPI : `detail` est une chaîne sur 400/404/
/// 409/500, ou un tableau structuré sur 422.
pub(crate) fn parse_detail(text: &str) -> Option<String> {
    let value: Value = serde_json::from_str(text).ok()?;
    let detail = value.get("detail")?;
    if let Some(message) = detail.as_str() {
        return Some(message.to_string());
    }
    if let Some(items) = detail.as_array() {
        let messages: Vec<String> = items.iter().filter_map(format_validation_error).collect();
        if !messages.is_empty() {
            return Some(messages.join(" · "));
        }
    }
    Some(detail.to_string())
}

fn format_validation_error(item: &Value) -> Option<String> {
    let message = item
        .get("msg")
        .and_then(Value::as_str)
        .unwrap_or("valeur invalide");
    let location = item
        .get("loc")
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| {
                    part.as_str()
                        .map(str::to_string)
                        .or_else(|| part.as_i64().map(|value| value.to_string()))
                })
                .collect::<Vec<_>>()
                .join(".")
        })
        .unwrap_or_default();
    if location.is_empty() {
        Some(message.to_string())
    } else {
        Some(format!("{} : {}", location, message))
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut truncated: String = text.chars().take(max).collect();
        truncated.push('…');
        truncated
    }
}

/// Valide un identifiant de chemin (`[A-Za-z0-9_-]{1,64}`). Les identifiants
/// viennent du serveur ou de l'utilisateur ; refuser un `/` évite toute
/// injection dans l'URL.
pub(crate) fn validate_id(id: &str, what: &str) -> Result<(), String> {
    let valid = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if valid {
        Ok(())
    } else {
        Err(coded(
            codes::INVALID,
            format!("Identifiant de {} invalide : « {} ».", what, id),
        ))
    }
}

/// Concatène une base sans barre oblique finale et un chemin.
pub(crate) fn join_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    if path.starts_with('/') {
        format!("{}{}", base, path)
    } else {
        format!("{}/{}", base, path)
    }
}
