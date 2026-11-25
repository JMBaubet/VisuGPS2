//! Ping **système** (ICMP) de l'hôte hermes.
//!
//! Il est distinct du `GET /ping` de l'API : il teste la machine et le réseau
//! privé (Tailscale), même si le service `hermes-api` est arrêté. Le store
//! l'exécute périodiquement pendant toute la durée de l'application.
//!
//! L'implémentation délègue au binaire `ping` du système : aucun privilège
//! particulier n'est requis, et le comportement reste celui de l'OS.

use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;

/// Budget global du ping (le binaire attend lui-même une réponse ~2 s).
const PING_TIMEOUT: Duration = Duration::from_secs(3);

/// Extrait l'hôte d'une URL de base (`http://100.118.62.10:9000/api/v1`).
///
/// Retourne `None` pour une URL sans schéma, sans hôte ou avec un hôte
/// inattendu. L'hôte est passé en argument (jamais via un shell).
pub(crate) fn host_from_base_url(base_url: &str) -> Option<String> {
    let (_, rest) = base_url.split_once("://")?;
    let authority = rest.split('/').next()?.trim();
    if authority.is_empty() {
        return None;
    }
    // Retire un éventuel `user:pass@`.
    let host_port = authority.rsplit('@').next()?;
    let host = if let Some(stripped) = host_port.strip_prefix('[') {
        // IPv6 littéral : [fd00::1]:9000
        stripped.split(']').next()?.to_string()
    } else {
        // Nom d'hôte ou IPv4, avec un port optionnel.
        host_port.split(':').next()?.to_string()
    };
    let host = host.trim();
    if host.is_empty() || host.starts_with('-') {
        return None;
    }
    if !host
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '_'))
    {
        return None;
    }
    Some(host.to_string())
}

#[cfg(target_os = "windows")]
fn build_command(host: &str) -> Command {
    let mut command = Command::new("ping");
    command.arg(if host.contains(':') { "-6" } else { "-4" });
    command.args(["-n", "1", "-w", "2000", host]);
    command
}

#[cfg(target_os = "macos")]
fn build_command(host: &str) -> Command {
    let ipv6 = host.contains(':');
    let mut command = Command::new(if ipv6 { "ping6" } else { "ping" });
    command.args(["-c", "1"]);
    // `-W` est en millisecondes sur macOS et non supporté par `ping6`.
    if !ipv6 {
        command.args(["-W", "2000"]);
    }
    command.arg(host);
    command
}

#[cfg(all(unix, not(target_os = "macos")))]
fn build_command(host: &str) -> Command {
    let mut command = Command::new("ping");
    command.arg(if host.contains(':') { "-6" } else { "-4" });
    command.args(["-c", "1", "-W", "2", host]);
    command
}

/// Exécute un ping unique. `Ok(())` si le binaire renvoie un succès.
pub(crate) async fn ping_once(host: &str) -> Result<(), String> {
    let mut command = build_command(host);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|_| "Ping système : commande indisponible.".to_string())?;
    match timeout(PING_TIMEOUT, child.wait()).await {
        Ok(Ok(status)) if status.success() => Ok(()),
        Ok(Ok(_)) => Err("Ping système : aucune réponse.".to_string()),
        Ok(Err(_)) => Err("Ping système : exécution en échec.".to_string()),
        Err(_) => {
            let _ = child.kill().await;
            Err("Ping système : délai dépassé.".to_string())
        }
    }
}
