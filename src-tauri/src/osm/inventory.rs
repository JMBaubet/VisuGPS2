//! Inventaire des fichiers PBF du support OSM.
//!
//! `build_overview` lit le dossier, `regions.json` et l'espace disque, puis
//! assemble le contrat IPC consommé par la toolbar et la fenêtre de gestion.
//! `file_status` est pure : elle reçoit la date de référence, ce qui permet de
//! tester les seuils (renouvellement, 90 % de validité) sans dépendre de
//! l'horloge.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Months, Utc};

use super::params::OsmSettings;
use super::types::{
    DATE_SOURCE_CREATED, DATE_SOURCE_MODIFIED, DATE_SOURCE_UNKNOWN, FILE_STATE_GREEN,
    FILE_STATE_ORANGE, FILE_STATE_YELLOW, OsmFileStatus, OsmSupportOverview, STATE_EMPTY,
    STATE_GREEN, STATE_NOT_CONFIGURED, STATE_ORANGE, STATE_USB_MISSING, STATE_YELLOW,
};
use crate::gpx_tunnels::regions;
use crate::gpx_tunnels::types::RegionEntry;

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
        if path.extension().and_then(|extension| extension.to_str()) != Some("pbf") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let Some(filename) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let (created, date_source) = file_date(&metadata);
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
        let a_name = a
            .region_name
            .as_deref()
            .unwrap_or(&a.filename)
            .to_lowercase();
        let b_name = b
            .region_name
            .as_deref()
            .unwrap_or(&b.filename)
            .to_lowercase();
        a_name
            .cmp(&b_name)
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

/// État global : l'orange prime sur le jaune, qui prime sur le vert.
pub fn overall_state(files: &[OsmFileStatus]) -> &'static str {
    if files.is_empty() {
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

/// Date de création du fichier, avec repli sur la modification puis l'inconnu.
fn file_date(metadata: &std::fs::Metadata) -> (Option<DateTime<Utc>>, &'static str) {
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
