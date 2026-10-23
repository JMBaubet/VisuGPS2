//! Tests de l'inventaire OSM : seuils d'état, listing et espace disque.

use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, TimeZone, Utc};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use osmpbfreader::fileformat::{Blob, BlobHeader};
use osmpbfreader::osmformat::HeaderBlock;
use protobuf::Message;
use uuid::Uuid;

use crate::gpx_tunnels::regions;
use crate::gpx_tunnels::types::{RegionEntry, RegionsFile};
use crate::osm::inventory::{
    build_overview, file_status, overall_state, pbf_generation_date,
};
use crate::osm::params::OsmSettings;
use crate::osm::types::{
    DATE_SOURCE_CREATED, DATE_SOURCE_UNKNOWN, FILE_STATE_GREEN, FILE_STATE_ORANGE,
    FILE_STATE_YELLOW, OsmFileStatus, STATE_EMPTY, STATE_GREEN, STATE_NOT_CONFIGURED,
    STATE_ORANGE, STATE_USB_MISSING, STATE_YELLOW,
};

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 23, 12, 0, 0).unwrap()
}

fn temp_dir() -> PathBuf {
    std::env::temp_dir().join(format!("visugps2-osm-{}", Uuid::new_v4()))
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn region(filename: &str) -> RegionEntry {
    RegionEntry {
        id: "provence-alpes-cote-d-azur".to_string(),
        name: "Provence-Alpes-Côte d'Azur".to_string(),
        country: "france".to_string(),
        parent: "france".to_string(),
        country_code: Some("FR".to_string()),
        country_name: Some("France".to_string()),
        continent: Some("europe".to_string()),
        continent_name: Some("Europe".to_string()),
        bbox: [4.65, 42.98, 7.72, 45.13],
        pbf_filename: filename.to_string(),
        pbf_url: format!("https://example/{}", filename),
        size_mb: None,
    }
}

fn status(created: Option<DateTime<Utc>>, settings: &OsmSettings) -> OsmFileStatus {
    let date_source = if created.is_some() {
        DATE_SOURCE_CREATED
    } else {
        DATE_SOURCE_UNKNOWN
    };
    file_status(
        "a.osm.pbf",
        1,
        created,
        date_source,
        None,
        settings,
        now(),
    )
}

#[test]
fn green_until_the_renewal_deadline() {
    let settings = OsmSettings::default();

    let green = status(Some(now() - Duration::days(55)), &settings);
    assert_eq!(green.state, FILE_STATE_GREEN);
    assert_eq!(green.days_remaining, 1);

    let due = status(Some(now() - Duration::days(56)), &settings);
    assert_eq!(due.state, FILE_STATE_YELLOW);
    assert_eq!(due.days_remaining, 0);

    let late = status(Some(now() - Duration::days(57)), &settings);
    assert_eq!(late.state, FILE_STATE_YELLOW);
    assert_eq!(late.days_remaining, -1);
}

#[test]
fn orange_starts_at_ninety_percent_of_validity() {
    // Renouvellement très long pour isoler le seuil de fin de vie.
    let settings = OsmSettings {
        renewal_weeks: 52,
        ..OsmSettings::default()
    };

    let before = status(Some(now() - Duration::days(328)), &settings);
    assert_eq!(before.state, FILE_STATE_GREEN);
    assert!(before.validity_days_remaining.unwrap() > 0);

    let at = status(Some(now() - Duration::days(329)), &settings);
    assert_eq!(at.state, FILE_STATE_ORANGE);
    assert!(at.validity_days_remaining.unwrap() > 0);
}

#[test]
fn expired_files_are_orange() {
    let settings = OsmSettings::default();
    let expired = status(Some(now() - Duration::days(400)), &settings);
    assert_eq!(expired.state, FILE_STATE_ORANGE);
    assert!(expired.validity_days_remaining.unwrap() < 0);
}

#[test]
fn unknown_date_is_orange() {
    let settings = OsmSettings::default();
    let unknown = status(None, &settings);
    assert_eq!(unknown.state, FILE_STATE_ORANGE);
    assert_eq!(unknown.created_at, None);
    assert_eq!(unknown.validity_days_remaining, None);
    assert_eq!(unknown.date_source, DATE_SOURCE_UNKNOWN);
}

#[test]
fn overall_state_prefers_orange_then_yellow() {
    let settings = OsmSettings::default();
    let green = status(Some(now() - Duration::days(1)), &settings);
    let yellow = status(Some(now() - Duration::days(60)), &settings);
    let orange = status(Some(now() - Duration::days(400)), &settings);

    assert_eq!(overall_state(&[]), STATE_EMPTY);
    assert_eq!(overall_state(std::slice::from_ref(&green)), STATE_GREEN);
    assert_eq!(
        overall_state(&[green.clone(), yellow.clone()]),
        STATE_YELLOW
    );
    assert_eq!(overall_state(&[green, yellow, orange]), STATE_ORANGE);
}

#[test]
fn build_overview_lists_pbf_files_and_resolves_regions() {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).expect("dossier temporaire");
    std::fs::write(dir.join("paca-latest.osm.pbf"), b"abc").expect("PBF");
    std::fs::write(dir.join("kenya-latest.osm.pbf"), b"def").expect("PBF Kenya");
    std::fs::write(dir.join("._paca-latest.osm.pbf"), b"hidden")
        .expect("métadonnées macOS");
    std::fs::write(dir.join("ignore.osm.pbf.part"), b"part").expect("partiel");
    std::fs::write(dir.join("notes.txt"), b"notes").expect("texte");

    let mut kenya = region("kenya-latest.osm.pbf");
    kenya.id = "kenya".to_string();
    kenya.name = "Kenya".to_string();
    kenya.country = "africa".to_string();
    kenya.parent = "africa".to_string();
    kenya.country_code = Some("KE".to_string());
    kenya.country_name = Some("Kenya".to_string());
    kenya.continent = Some("africa".to_string());
    kenya.continent_name = Some("Afrique".to_string());
    kenya.bbox = [33.0, -5.0, 42.0, 5.0];

    regions::save_regions(
        &dir,
        &RegionsFile {
            version: "v1".to_string(),
            regions: vec![region("paca-latest.osm.pbf"), kenya],
        },
    )
    .expect("regions.json");

    let settings = OsmSettings {
        pbf_directory: Some(dir.clone()),
        ..OsmSettings::default()
    };
    let overview = build_overview(&settings, Utc::now());

    assert!(overview.configured);
    assert!(overview.readable);
    assert!(!overview.usb_missing);
    assert!(overview.regions_present);
    assert_eq!(overview.regions_version.as_deref(), Some("v1"));
    assert_eq!(overview.files.len(), 2, "les ._*.pbf doivent être ignorés");

    let file = &overview.files[0];
    assert_eq!(file.filename, "paca-latest.osm.pbf");
    assert_eq!(
        file.region_id.as_deref(),
        Some("provence-alpes-cote-d-azur")
    );
    assert_eq!(file.country_code.as_deref(), Some("FR"));
    assert_eq!(file.country_name.as_deref(), Some("France"));
    assert_eq!(file.continent.as_deref(), Some("europe"));
    assert_eq!(file.continent_name.as_deref(), Some("Europe"));
    assert_eq!(file.size_bytes, 3);
    assert_eq!(file.state, FILE_STATE_GREEN);

    let kenya = &overview.files[1];
    assert_eq!(kenya.filename, "kenya-latest.osm.pbf");
    assert_eq!(kenya.continent.as_deref(), Some("africa"));
    assert_eq!(kenya.continent_name.as_deref(), Some("Afrique"));

    assert_eq!(overview.total_size_bytes, 6);
    assert!(overview.free_bytes.is_some());
    assert!(overview.total_bytes.is_some());
    assert_eq!(overview.state, STATE_GREEN);

    cleanup(&dir);
}

#[test]
fn build_overview_reports_not_configured_and_usb_missing() {
    let overview = build_overview(&OsmSettings::default(), Utc::now());
    assert!(!overview.configured);
    assert!(!overview.readable);
    assert!(!overview.usb_missing);
    assert_eq!(overview.state, STATE_NOT_CONFIGURED);
    assert!(overview.message.is_some());

    let missing =
        std::env::temp_dir().join(format!("visugps2-osm-absent-{}", Uuid::new_v4()));
    let settings = OsmSettings {
        pbf_directory: Some(missing),
        ..OsmSettings::default()
    };
    let overview = build_overview(&settings, Utc::now());
    assert!(overview.configured);
    assert!(!overview.readable);
    assert!(overview.usb_missing);
    assert_eq!(overview.state, STATE_USB_MISSING);
    assert!(overview
        .message
        .as_deref()
        .unwrap_or("")
        .contains("Support OSM introuvable"));
}

#[test]
fn build_overview_reports_empty_folder() {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).expect("dossier temporaire");
    let settings = OsmSettings {
        pbf_directory: Some(dir.clone()),
        ..OsmSettings::default()
    };
    let overview = build_overview(&settings, Utc::now());

    assert!(overview.readable);
    assert!(overview.files.is_empty());
    assert_eq!(overview.state, STATE_EMPTY);
    assert_eq!(overview.total_size_bytes, 0);
    assert!(overview
        .message
        .as_deref()
        .unwrap_or("")
        .contains("Aucun fichier PBF"));

    cleanup(&dir);
}

fn write_pbf_with_generation_date(path: &Path, timestamp: i64, compressed: bool) {
    let mut header_block = HeaderBlock::new();
    header_block.set_osmosis_replication_timestamp(timestamp);
    let payload = header_block.write_to_bytes().expect("HeaderBlock");

    let mut blob = Blob::new();
    if compressed {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&payload).expect("compression zlib");
        let compressed_payload = encoder.finish().expect("fin zlib");
        blob.set_raw_size(payload.len() as i32);
        blob.set_zlib_data(compressed_payload);
    } else {
        blob.set_raw(payload);
    }
    let blob_bytes = blob.write_to_bytes().expect("Blob");

    let mut blob_header = BlobHeader::new();
    blob_header.set_type("OSMHeader".to_string());
    blob_header.set_datasize(blob_bytes.len() as i32);
    let header_bytes = blob_header.write_to_bytes().expect("BlobHeader");

    let mut file = std::fs::File::create(path).expect("fichier PBF");
    file.write_all(&(header_bytes.len() as u32).to_be_bytes())
        .expect("longueur du BlobHeader");
    file.write_all(&header_bytes).expect("BlobHeader");
    file.write_all(&blob_bytes).expect("Blob");
}

#[test]
fn pbf_generation_date_is_read_from_the_header() {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).expect("dossier temporaire");
    let path = dir.join("monaco-latest.osm.pbf");
    let expected = DateTime::from_timestamp(1_700_000_000, 0).expect("horodatage");

    write_pbf_with_generation_date(&path, 1_700_000_000, false);
    assert_eq!(pbf_generation_date(&path), Some(expected));

    write_pbf_with_generation_date(&path, 1_700_000_000, true);
    assert_eq!(pbf_generation_date(&path), Some(expected));

    cleanup(&dir);
}
