//! Téléchargements Geofabrik et préparation de `regions.json`.
//!
//! Les téléchargements sont **silencieux** : aucune confirmation, aucune
//! fenêtre modale. La progression est remontée par un callback que la couche
//! Tauri transforme en événement `tunnels-progress`.
//!
//! Les écritures sont atomiques : le flux va dans `<fichier>.part`, puis le
//! fichier est renommé. Un échec supprime le `.part` et laisse la cible
//! précédente intacte.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::io::AsyncWriteExt;

use super::regions;
use super::types::{RegionEntry, RegionsFile};

/// Délai de connexion HTTP.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Client HTTP commun. TLS portable (rustls), suivi des redirections par
/// défaut.
pub fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(format!("VisuGPS2/{}", env!("CARGO_PKG_VERSION")))
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .map_err(|e| format!("Création du client HTTP : {}", e))
}

/// Taille distante d'un fichier, via une requête HEAD. `None` si le serveur
/// refuse HEAD ou ne fournit pas `Content-Length`.
pub async fn remote_size(client: &reqwest::Client, url: &str) -> Option<u64> {
    let response = client.head(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    // `Response::content_length()` peut valoir 0 pour une réponse HEAD (corps
    // vide) ; on lit directement l'en-tête annoncé par le serveur.
    response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
}

/// Chemin du fichier temporaire d'un téléchargement.
pub fn part_path(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download");
    target.with_file_name(format!("{}.part", name))
}

/// Télécharge `url` vers `target` avec écriture atomique et progression.
///
/// `on_progress(done, total)` est appelé au plus une fois par pourcentage (ou
/// par mégaoctet quand la taille est inconnue), et systématiquement à la fin.
pub async fn download_to_file<F>(
    client: &reqwest::Client,
    url: &str,
    target: &Path,
    on_progress: F,
) -> Result<(), String>
where
    F: Fn(u64, u64) + Send + Sync,
{
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Création du dossier de téléchargement : {}", e))?;
    }

    let part = part_path(target);
    let _ = tokio::fs::remove_file(&part).await;

    if let Err(error) = stream_to_part(client, url, &part, &on_progress).await {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(error);
    }

    // Windows refuse le renommage sur une cible existante.
    if tokio::fs::metadata(target).await.is_ok() {
        let _ = tokio::fs::remove_file(target).await;
    }
    tokio::fs::rename(&part, target).await.map_err(|e| {
        format!(
            "Installation du fichier téléchargé ({}): {}",
            target.display(),
            e
        )
    })?;

    Ok(())
}

async fn stream_to_part<F>(
    client: &reqwest::Client,
    url: &str,
    part: &Path,
    on_progress: &F,
) -> Result<(), String>
where
    F: Fn(u64, u64) + Send + Sync,
{
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Téléchargement impossible ({}): {}", url, e))?;
    if !response.status().is_success() {
        return Err(format!(
            "Téléchargement refusé ({}) : HTTP {}.",
            url,
            response.status()
        ));
    }

    let total = response.content_length().unwrap_or(0);
    let mut file = tokio::fs::File::create(part)
        .await
        .map_err(|e| format!("Création du fichier temporaire ({}): {}", part.display(), e))?;
    let mut done = 0_u64;
    let mut last_reported = 0_u64;

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Lecture du flux ({}): {}", url, e))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Écriture du téléchargement ({}): {}", part.display(), e))?;
        done += chunk.len() as u64;
        if should_report(done, total, last_reported) {
            on_progress(done, total);
            last_reported = done;
        }
    }

    file.flush()
        .await
        .map_err(|e| format!("Finalisation du téléchargement ({}): {}", part.display(), e))?;
    on_progress(done, total);
    Ok(())
}

fn should_report(done: u64, total: u64, last_reported: u64) -> bool {
    if done == total {
        return true;
    }
    let step = if total > 0 {
        (total / 100).max(1)
    } else {
        1_000_000
    };
    done.saturating_sub(last_reported) >= step
}

/// Assure la présence de `regions.json` dans le dossier OSM.
///
/// Si le fichier est absent ou illisible, l'index Geofabrik est téléchargé dans
/// le dossier temporaire du système, transformé, puis `regions.json` est écrit
/// atomiquement sur le support OSM. Le fichier temporaire est toujours
/// supprimé.
pub async fn ensure_regions_json<F>(
    client: &reqwest::Client,
    pbf_dir: &Path,
    index_url: &str,
    on_progress: F,
) -> Result<RegionsFile, String>
where
    F: Fn(u64, u64) + Send + Sync,
{
    if let Some(regions) = regions::load_regions(pbf_dir) {
        return Ok(regions);
    }
    if !pbf_dir.is_dir() {
        return Err(format!(
            "Dossier OSM introuvable : {}.",
            pbf_dir.display()
        ));
    }

    let temp = std::env::temp_dir().join(format!(
        "visugps2-index-{}.json",
        uuid::Uuid::new_v4()
    ));

    if let Err(error) = download_to_file(client, index_url, &temp, on_progress).await {
        let _ = tokio::fs::remove_file(&temp).await;
        return Err(error);
    }

    let content = tokio::fs::read_to_string(&temp).await;
    let _ = tokio::fs::remove_file(&temp).await;
    let content = content.map_err(|e| {
        format!(
            "Lecture de l'index Geofabrik téléchargé ({}): {}",
            temp.display(),
            e
        )
    })?;

    let regions = regions::transform_index(
        &content,
        &crate::gpx_audit::export::iso_now(),
    )?;
    regions::save_regions(pbf_dir, &regions)?;
    Ok(regions)
}

/// Télécharge un PBF régional s'il n'est pas déjà présent.
pub async fn download_pbf<F>(
    client: &reqwest::Client,
    region: &RegionEntry,
    pbf_dir: &Path,
    on_progress: F,
) -> Result<PathBuf, String>
where
    F: Fn(u64, u64) + Send + Sync,
{
    let target = regions::pbf_path(pbf_dir, &region.pbf_filename);
    if regions::pbf_present(pbf_dir, &region.pbf_filename) {
        return Ok(target);
    }
    if region.pbf_url.is_empty() {
        return Err(format!(
            "URL du PBF absente pour la région {}.",
            region.id
        ));
    }

    download_to_file(client, &region.pbf_url, &target, on_progress).await?;
    Ok(target)
}
