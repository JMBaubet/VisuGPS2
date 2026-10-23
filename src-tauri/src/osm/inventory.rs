//! Inventaire des fichiers PBF du support OSM.
//!
//! `build_overview` lit le dossier, `regions.json` et l'espace disque, puis
//! assemble le contrat IPC consommé par la toolbar et la fenêtre de gestion.
//! `file_status` est pure : elle reçoit la date de référence, ce qui permet de
//! tester les seuils (renouvellement, 90 % de validité) sans dépendre de
//! l'horloge.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use chrono::{DateTime, Duration, Months, Utc};
use flate2::read::ZlibDecoder;
use osmpbfreader::fileformat::{Blob, BlobHeader};
use osmpbfreader::osmformat::HeaderBlock;
use protobuf::Message;

use super::params::OsmSettings;
use super::types::{
    DATE_SOURCE_CREATED, DATE_SOURCE_MODIFIED, DATE_SOURCE_PBF, DATE_SOURCE_UNKNOWN,
    FILE_STATE_GREEN, FILE_STATE_ORANGE, FILE_STATE_YELLOW, OsmFileStatus,
    OsmSupportOverview, STATE_EMPTY, STATE_GREEN, STATE_NOT_CONFIGURED, STATE_ORANGE,
    STATE_USB_MISSING, STATE_YELLOW,
};
use crate::gpx_tunnels::regions;
use crate::gpx_tunnels::types::RegionEntry;

/// Taille maximale d'un en-tête de blob PBF (spécification OSM PBF).
const MAX_BLOB_HEADER_SIZE: u32 = 64 * 1024;
/// Taille maximale acceptée pour le bloc d'en-tête décompressé.
const MAX_HEADER_BLOB_SIZE: u32 = 32 * 1024 * 1024;

/// Construit l'état complet du support OSM à l'instant `now`.
pub fn build_overview(settings: &OsmSettings, now: DateTime<Utc>) -> OsmSupportOverview {
    let Some(folder) = settings.pbf_directory.as_deref() else {
        return empty_overview(
            settings,
            None,
            false,
            false,
            STATE_NOT_CONFIGURED,
            "Aucun dossier OSM n'est configuré.".to_string(),
        );
    };

    let folder_string = folder.to_string_lossy().to_string();
    let entries = match std::fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(_) => {
            return empty_overview(
                settings,
                Some(folder_string.clone()),
                true,
                false,
                STATE_USB_MISSING,
                format!(
                    "Support OSM introuvable : {}. Vérifiez que la clé USB est branchée.",
                    folder_string
                ),
            );
        }
    };

    let regions_file = regions::load_regions(folder);
    let regions_present = regions_file.is_some();
    let regions_version = regions_file
        .as_ref()
        .map(|file| file.version.clone());
    let regions_by_filename: HashMap<&str, &RegionEntry> = regions_file
        .as_ref()
        .map(|file| {
            file.regions
                .iter()
                .map(|region| (region.pbf_filename.as_str(), region))
                .collect()
        })
        .unwrap_or_default();

    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(filename) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !regions::is_pbf_filename(filename) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let (created, date_source) = file_date(&path, &metadata);
        let region = regions_by_filename.get(filename).copied();
        files.push(file_status(
            filename,
            metadata.len(),
            created,
            date_source,
            region,
            settings,
            now,
        ));
    }

    files.sort_by(|a, b| {
        continent_rank(a.continent.as_deref())
            .cmp(&continent_rank(b.continent.as_deref()))
            .then_with(|| {
                a.continent_name
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .cmp(
                        &b.continent_name
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase(),
                    )
            })
            .then_with(|| {
                a.country_name
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .cmp(&b.country_name.as_deref().unwrap_or("").to_lowercase())
            })
            .then_with(|| {
                a.region_name
                    .as_deref()
                    .unwrap_or(a.filename.as_str())
                    .to_lowercase()
                    .cmp(
                        &b.region_name
                            .as_deref()
                            .unwrap_or(b.filename.as_str())
                            .to_lowercase(),
                    )
            })
            .then_with(|| a.filename.cmp(&b.filename))
    });

    let total_size_bytes = files
        .iter()
        .fold(0_u64, |total, file| total.saturating_add(file.size_bytes));
    let state = overall_state(&files).to_string();
    let free_bytes = fs4::available_space(folder).ok();
    let total_bytes = fs4::total_space(folder).ok();
    let message = files
        .is_empty()
        .then(|| "Aucun fichier PBF présent.".to_string());

    OsmSupportOverview {
        configured: true,
        readable: true,
        usb_missing: false,
        folder: Some(folder_string),
        regions_present,
        regions_version,
        free_bytes,
        total_bytes,
        max_pbf_mb: settings.max_pbf_mb,
        max_validity_months: settings.max_validity_months,
        renewal_weeks: settings.renewal_weeks,
        state,
        files,
        total_size_bytes,
        message,
    }
}

/// Calcule l'état d'un fichier à partir de sa date de référence.
///
/// Le renouvellement est dû à `création + renewal_weeks` ; la fin de vie est à
/// `création + max_validity_months`, et l'orange est atteint à 90 % de cette
/// durée. Une date inconnue est traitée comme orange : l'âge ne peut pas être
/// garanti.
pub fn file_status(
    filename: &str,
    size_bytes: u64,
    created: Option<DateTime<Utc>>,
    date_source: &str,
    region: Option<&RegionEntry>,
    settings: &OsmSettings,
    now: DateTime<Utc>,
) -> OsmFileStatus {
    let renewal_days = (settings.renewal_weeks as i64) * 7;
    let (created_at, age_days, days_remaining, validity_days_remaining, state) = match created {
        Some(created) => {
            let age_days = now.signed_duration_since(created).num_days().max(0);
            let days_remaining = renewal_days - age_days;
            let validity_end = created
                .checked_add_months(Months::new(settings.max_validity_months))
                .unwrap_or_else(|| {
                    created + Duration::days(365 * settings.max_validity_months as i64)
                });
            let validity_days_remaining = (validity_end - now).num_days();
            let life_seconds = (validity_end - created).num_seconds().max(1);
            let elapsed_seconds = (now - created).num_seconds();
            let orange = elapsed_seconds >= 0
                && elapsed_seconds.saturating_mul(10) >= life_seconds.saturating_mul(9);
            let state = if orange {
                FILE_STATE_ORANGE
            } else if age_days >= renewal_days {
                FILE_STATE_YELLOW
            } else {
                FILE_STATE_GREEN
            };
            (
                Some(created.to_rfc3339()),
                age_days,
                days_remaining,
                Some(validity_days_remaining),
                state,
            )
        }
        None => (None, 0, 0, None, FILE_STATE_ORANGE),
    };

    OsmFileStatus {
        filename: filename.to_string(),
        region_id: region.map(|entry| entry.id.clone()),
        region_name: region.map(|entry| entry.name.clone()),
        country_code: region.and_then(|entry| entry.country_code.clone()),
        country_name: region.and_then(|entry| entry.country_name.clone()),
        continent: region.and_then(|entry| entry.continent.clone()),
        continent_name: region.and_then(|entry| entry.continent_name.clone()),
        size_bytes,
        created_at,
        date_source: date_source.to_string(),
        age_days,
        renewal_days,
        days_remaining,
        validity_days_remaining,
        state: state.to_string(),
    }
}

/// Rang de tri des continents dans la liste des fichiers, aligné sur
/// l'arborescence de téléchargement (Europe en tête, inconnus en fin).
fn continent_rank(continent: Option<&str>) -> usize {
    match continent {
        Some("europe") => 0,
        Some("africa") => 1,
        Some("north-america") => 2,
        Some("south-america") => 3,
        Some("central-america") => 4,
        Some("asia") => 5,
        Some("australia-oceania") => 6,
        Some("antarctica") => 7,
        _ => 99,
    }
}

/// État global : l'orange prime sur le jaune, qui prime sur le vert.
pub fn overall_state(files: &[OsmFileStatus]) -> &'static str {    if files.is_empty() {
        return STATE_EMPTY;
    }
    if files
        .iter()
        .any(|file| file.state == FILE_STATE_ORANGE)
    {
        return STATE_ORANGE;
    }
    if files
        .iter()
        .any(|file| file.state == FILE_STATE_YELLOW)
    {
        return STATE_YELLOW;
    }
    STATE_GREEN
}

fn empty_overview(
    settings: &OsmSettings,
    folder: Option<String>,
    configured: bool,
    readable: bool,
    state: &str,
    message: String,
) -> OsmSupportOverview {
    OsmSupportOverview {
        configured,
        readable,
        usb_missing: configured && !readable,
        folder,
        regions_present: false,
        regions_version: None,
        free_bytes: None,
        total_bytes: None,
        max_pbf_mb: settings.max_pbf_mb,
        max_validity_months: settings.max_validity_months,
        renewal_weeks: settings.renewal_weeks,
        state: state.to_string(),
        files: Vec::new(),
        total_size_bytes: 0,
        message: Some(message),
    }
}

/// Date de génération OSM lue dans l'en-tête PBF
/// (`osmosis_replication_timestamp`), avec repli sur la date de création puis
/// de modification du fichier.
pub fn file_date(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> (Option<DateTime<Utc>>, &'static str) {
    if let Some(generated) = pbf_generation_date(path) {
        return (Some(generated), DATE_SOURCE_PBF);
    }
    if let Ok(created) = metadata.created() {
        return (
            Some(DateTime::<Utc>::from(created)),
            DATE_SOURCE_CREATED,
        );
    }
    if let Ok(modified) = metadata.modified() {
        return (
            Some(DateTime::<Utc>::from(modified)),
            DATE_SOURCE_MODIFIED,
        );
    }
    (None, DATE_SOURCE_UNKNOWN)
}

/// Lit `osmosis_replication_timestamp` dans le premier bloc `OSMHeader`.
///
/// Le PBF est composé d'un `BlobHeader` (4 octets de longueur big-endian, puis
/// message protobuf) et d'un `Blob` (brut ou compressé zlib). Le timestamp est
/// la date de génération des données OSM, indépendante de la date de copie du
/// fichier. Retourne `None` si le fichier n'est pas lisible, n'est pas un PBF
/// ou ne porte pas l'horodatage (l'appelant retombe alors sur le système de
/// fichiers).
pub fn pbf_generation_date(path: &Path) -> Option<DateTime<Utc>> {
    let mut file = File::open(path).ok()?;
    let mut length_bytes = [0_u8; 4];
    file.read_exact(&mut length_bytes).ok()?;
    let header_length = u32::from_be_bytes(length_bytes);
    if header_length == 0 || header_length > MAX_BLOB_HEADER_SIZE {
        return None;
    }

    let mut header_bytes = vec![0_u8; header_length as usize];
    file.read_exact(&mut header_bytes).ok()?;
    let header = BlobHeader::parse_from_bytes(&header_bytes).ok()?;
    if header.type_() != "OSMHeader" {
        return None;
    }

    let datasize = header.datasize();
    if datasize <= 0 || datasize as u32 > MAX_HEADER_BLOB_SIZE {
        return None;
    }
    let mut blob_bytes = vec![0_u8; datasize as usize];
    file.read_exact(&mut blob_bytes).ok()?;
    let blob = Blob::parse_from_bytes(&blob_bytes).ok()?;
    let payload = decode_blob(&blob)?;

    let header_block = HeaderBlock::parse_from_bytes(&payload).ok()?;
    if !header_block.has_osmosis_replication_timestamp() {
        return None;
    }
    let timestamp = header_block.osmosis_replication_timestamp();
    if timestamp <= 0 {
        return None;
    }
    DateTime::from_timestamp(timestamp, 0)
}

/// Décompresse un `Blob` PBF (brut ou zlib), avec une borne de sécurité.
fn decode_blob(blob: &Blob) -> Option<Vec<u8>> {
    let raw = blob.raw();
    if !raw.is_empty() {
        return Some(raw.to_vec());
    }

    let zlib = blob.zlib_data();
    if zlib.is_empty() {
        return None;
    }
    let limit = if blob.raw_size() > 0 {
        (blob.raw_size() as u32).min(MAX_HEADER_BLOB_SIZE)
    } else {
        MAX_HEADER_BLOB_SIZE
    };
    let mut decoder = ZlibDecoder::new(zlib);
    let mut output = Vec::new();
    decoder
        .by_ref()
        .take(limit as u64)
        .read_to_end(&mut output)
        .ok()?;
    Some(output)
}
