//! Tests des téléchargements HTTP, sans réseau externe.
//!
//! Un petit serveur TCP local répond une fois par test. Il permet de vérifier
//! l'écriture atomique `.part` → cible, la progression et les erreurs HTTP.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use crate::gpx_tunnels::download;
use crate::gpx_tunnels::regions;
use crate::gpx_tunnels::types::{RegionEntry, RegionsFile};

fn temp_dir() -> PathBuf {
    std::env::temp_dir().join(format!("visugps2-download-{}", Uuid::new_v4()))
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn serve_once(status: &'static str, content_type: &'static str, body: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("port local");
    let address = listener.local_addr().expect("adresse locale");
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("connexion de test");
        let mut request = [0_u8; 2048];
        let _ = stream.read(&mut request);
        let header = format!(
            "HTTP/1.1 {}\r\nContent-Length: {}\r\nContent-Type: {}\r\nConnection: close\r\n\r\n",
            status,
            body.len(),
            content_type
        );
        stream
            .write_all(header.as_bytes())
            .expect("en-tête de test");
        stream.write_all(&body).expect("corps de test");
        let _ = stream.flush();
    });
    format!("http://{}/file", address)
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime tokio")
        .block_on(future)
}

#[test]
fn download_writes_the_target_atomically_and_reports_progress() {
    let url = serve_once("200 OK", "application/octet-stream", b"hello".to_vec());
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).expect("dossier temporaire");
    let target = dir.join("monaco-latest.osm.pbf");
    let progress: Arc<Mutex<Vec<(u64, u64)>>> = Arc::new(Mutex::new(Vec::new()));
    let observer = Arc::clone(&progress);
    let client = download::client().expect("client HTTP");

    block_on(download::download_to_file(
        &client,
        &url,
        &target,
        move |done, total| {
            observer.lock().expect("progression").push((done, total));
        },
    ))
    .expect("téléchargement");

    assert_eq!(std::fs::read(&target).expect("contenu"), b"hello");
    assert!(
        !download::part_path(&target).exists(),
        "aucun .part ne doit subsister"
    );
    assert_eq!(
        progress.lock().expect("progression").last(),
        Some(&(5, 5))
    );

    cleanup(&dir);
}

#[test]
fn http_error_leaves_no_target_and_no_part() {
    let url = serve_once("404 Not Found", "text/plain", b"absent".to_vec());
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).expect("dossier temporaire");
    let target = dir.join("absent.osm.pbf");
    let client = download::client().expect("client HTTP");

    let result = block_on(download::download_to_file(
        &client,
        &url,
        &target,
        |_, _| {},
    ));

    assert!(result.is_err());
    assert!(!target.exists());
    assert!(!download::part_path(&target).exists());

    cleanup(&dir);
}

#[test]
fn remote_size_reads_content_length() {
    let url = serve_once("200 OK", "application/octet-stream", b"hello".to_vec());
    let client = download::client().expect("client HTTP");

    let size = block_on(download::remote_size(&client, &url));
    assert_eq!(size, Some(5));
}

#[test]
fn remote_size_returns_none_on_http_error() {
    let url = serve_once("404 Not Found", "text/plain", b"absent".to_vec());
    let client = download::client().expect("client HTTP");

    assert_eq!(block_on(download::remote_size(&client, &url)), None);
}

#[test]
fn ensure_regions_json_downloads_transforms_and_writes_atomically() {
    let index = r#"{
      "type": "FeatureCollection",
      "features": [{
        "type": "Feature",
        "geometry": {
          "type": "Polygon",
          "coordinates": [[[7.40, 43.72], [7.44, 43.72], [7.44, 43.75], [7.40, 43.75], [7.40, 43.72]]]
        },
        "properties": {
          "id": "monaco",
          "name": "Monaco",
          "parent": "europe",
          "iso3166-1:alpha2": ["MC"],
          "urls": { "pbf": "https://example/monaco-latest.osm.pbf" }
        }
      }]
    }"#;
    let url = serve_once("200 OK", "application/json", index.as_bytes().to_vec());
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).expect("dossier temporaire");
    let client = download::client().expect("client HTTP");

    let regions = block_on(download::ensure_regions_json(
        &client,
        &dir,
        &url,
        |_, _| {},
    ))
    .expect("préparation de regions.json");

    assert_eq!(regions.regions.len(), 1);
    assert_eq!(regions.regions[0].id, "monaco");
    assert_eq!(regions.regions[0].country_code.as_deref(), Some("MC"));
    assert_eq!(regions.regions[0].country_name.as_deref(), Some("Monaco"));
    assert!(regions::regions_path(&dir).exists());
    let loaded = regions::load_regions(&dir).expect("regions.json relu");
    assert_eq!(loaded, regions);

    cleanup(&dir);
}

#[test]
fn download_pbf_skips_an_existing_file_and_writes_a_missing_one() {
    let url = serve_once("200 OK", "application/octet-stream", b"pbf".to_vec());
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).expect("dossier temporaire");
    let region = RegionEntry {
        id: "europe/monaco".to_string(),
        name: "Monaco".to_string(),
        country: "europe".to_string(),
        parent: "europe".to_string(),
        country_code: None,
        country_name: None,
        bbox: [7.40, 43.72, 7.44, 43.75],
        pbf_filename: "monaco-latest.osm.pbf".to_string(),
        pbf_url: url,
        size_mb: None,
    };
    let client = download::client().expect("client HTTP");

    let target = block_on(download::download_pbf(&client, &region, &dir, |_, _| {}))
        .expect("téléchargement du PBF");
    assert_eq!(std::fs::read(&target).expect("contenu"), b"pbf");
    assert!(regions::pbf_present(&dir, &region.pbf_filename));

    // Deuxième appel : le fichier existe, aucun nouveau téléchargement n'est
    // nécessaire (le serveur de test a déjà répondu une fois).
    let again = block_on(download::download_pbf(&client, &region, &dir, |_, _| {}))
        .expect("PBF déjà présent");
    assert_eq!(again, target);

    cleanup(&dir);
}

#[test]
fn regions_file_is_loaded_without_network_when_present() {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).expect("dossier temporaire");
    let initial = RegionsFile {
        version: "v1".to_string(),
        regions: vec![RegionEntry {
            id: "monaco".to_string(),
            name: "Monaco".to_string(),
            country: "europe".to_string(),
            parent: "europe".to_string(),
            country_code: Some("MC".to_string()),
            country_name: Some("Monaco".to_string()),
            bbox: [7.40, 43.72, 7.44, 43.75],
            pbf_filename: "monaco-latest.osm.pbf".to_string(),
            pbf_url: "https://example/monaco-latest.osm.pbf".to_string(),
            size_mb: None,
        }],
    };
    regions::save_regions(&dir, &initial).expect("regions.json initial");
    let client = download::client().expect("client HTTP");

    // URL invalide : si le fichier existait mal, le test échouerait par réseau.
    let loaded = block_on(download::ensure_regions_json(
        &client,
        &dir,
        "http://127.0.0.1:1/invalid",
        |_, _| {},
    ))
    .expect("chargement local");

    assert_eq!(loaded, initial);
    cleanup(&dir);
}

#[test]
#[ignore = "accès réseau : vérifie l'index Geofabrik réel (~50 Mo)"]
fn fetch_real_geofabrik_index() {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).expect("dossier temporaire");
    let client = download::client().expect("client HTTP");

    let regions = block_on(download::ensure_regions_json(
        &client,
        &dir,
        regions::INDEX_URL,
        |_, _| {},
    ))
    .expect("index Geofabrik réel");

    println!("[bench] {} régions Geofabrik", regions.regions.len());
    assert!(regions.regions.len() > 100);
    assert!(
        regions
            .regions
            .iter()
            .any(|region| region.name == "Monaco"),
        "Monaco doit figurer dans l'index Geofabrik"
    );

    cleanup(&dir);
}
