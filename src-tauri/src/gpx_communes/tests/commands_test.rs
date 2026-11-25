//! Tests des commandes du module Communes : écriture (détection) et diagnostic
//! (réinitialisation).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::gpx_communes::commands::{
    build_detection_request, commit_detection, commit_hermes_detection, default_communes_params,
    detect_impl, reset_impl,
};
use crate::gpx_communes::file::{file_path, load_file};
use crate::hermes::types::HermesCommunesDetectResponse;
use crate::import_gpx::{
    get_traces_path, load_registry, save_registry, Point3D, TraceMetadata, TraceStats,
};

// ─── Aides de test ────────────────────────────────────────────────────

/// Dossier temporaire isolé pour un test (nettoyé au préalable).
fn test_mode_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vg2_communes_cmd_{}_{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// Entrée de registre minimale : trace nettoyée, sans détection de communes.
fn make_trace(trace_id: &str) -> TraceMetadata {
    TraceMetadata {
        id: trace_id.to_string(),
        name: "Trace de test".to_string(),
        source: "Test".to_string(),
        source_url: None,
        activity_type: None,
        filename: "trace.gpx".to_string(),
        import_date: "2026-01-01T00:00:00Z".to_string(),
        stats: TraceStats {
            start_point: Point3D { lat: 45.0, lon: 2.0, alt: None },
            end_point: Point3D { lat: 45.0, lon: 2.02, alt: None },
            distance_m: 1574.0,
            positive_elevation_m: 0.0,
            negative_elevation_m: 0.0,
            alt_min_m: None,
            alt_max_m: None,
            points_count: 3,
            duration_s: None,
        },
        hash: "sha256:test".to_string(),
        favorite: false,
        is_displayed: false,
        audit_status: "clean".to_string(),
        audit_archived: false,
        multiride_status: None,
        multiride_complete: None,
        communes_completed: false,
        communes_status: None,
        altimetrie_validated: false,
        altimetrie_climb_count: None,
        tunnels_status: None,
        tunnels_count: None,
    }
}

/// Géométrie persistée d'une trace : LineString ouest → est à latitude constante
/// (`{mode}/traces/{id}/trace.geojson`, seul fichier lu par la détection).
fn write_trace_geojson(mode: &Path, trace_id: &str) {
    let dir = mode.join("traces").join(trace_id);
    fs::create_dir_all(&dir).expect("dossier de trace");
    let feature = json!({
        "type": "Feature",
        "properties": { "id": trace_id },
        "geometry": {
            "type": "LineString",
            "coordinates": [[2.0, 45.0], [2.01, 45.0], [2.02, 45.0]],
        },
    });
    fs::write(
        dir.join("trace.geojson"),
        serde_json::to_string(&feature).expect("sérialisation"),
    )
    .expect("écriture de la géométrie");
}

/// Réponse Overpass (tableau `elements`) : une commune couvrant toute la trace.
fn commune_payload(name: &str) -> String {
    let ring = json!([
        { "lat": 44.98, "lon": 1.99 },
        { "lat": 45.02, "lon": 1.99 },
        { "lat": 45.02, "lon": 2.03 },
        { "lat": 44.98, "lon": 2.03 },
        { "lat": 44.98, "lon": 1.99 },
    ]);
    serde_json::to_string(&json!([{
        "type": "relation",
        "id": 1,
        "tags": { "boundary": "administrative", "admin_level": "8", "name": name },
        "members": [{ "type": "way", "ref": 1, "role": "outer", "geometry": ring }],
    }]))
    .expect("sérialisation du payload")
}

/// Mode temporaire avec une trace : géométrie persistée + entrée de registre.
fn setup(name: &str, trace_id: &str) -> PathBuf {
    let mode = test_mode_dir(name);
    fs::create_dir_all(&mode).expect("dossier du mode");
    write_trace_geojson(&mode, trace_id);
    save_registry(&get_traces_path(&mode), &[make_trace(trace_id)]).expect("écriture du registre");
    mode
}

// ─── Détection ────────────────────────────────────────────────────────

/// La détection écrit la description **puis** pose le drapeau du registre.
#[test]
fn detect_writes_the_description_and_raises_the_flag() {
    let trace_id = "t-detect";
    let mode = setup("detect", trace_id);

    let passages = detect_impl(
        &mode,
        trace_id,
        &commune_payload("Test"),
        &default_communes_params(),
    )
    .expect("la détection doit aboutir");

    assert_eq!(passages.len(), 1);
    assert_eq!(passages[0].commune, "Test");
    assert_eq!(passages[0].km_entree, 0.0, "la trace démarre dans la commune");
    assert!(file_path(&mode, trace_id).exists(), "la description doit être écrite");
    assert_eq!(load_file(&mode, trace_id), Some(passages.clone()), "relisible telle quelle");
    assert!(
        load_registry(&get_traces_path(&mode))[0].communes_completed,
        "le drapeau doit être posé"
    );

    let _ = fs::remove_dir_all(&mode);
}

/// Une trace sans description ne rend rien à consulter — sans erreur.
#[test]
fn load_returns_nothing_without_a_description() {
    let trace_id = "t-none";
    let mode = setup("load_none", trace_id);

    assert_eq!(load_file(&mode, trace_id), None);

    let _ = fs::remove_dir_all(&mode);
}

// ─── Réinitialisation (diagnostic) ────────────────────────────────────

/// La réinitialisation supprime le fichier et repose le drapeau.
#[test]
fn reset_removes_the_file_and_clears_the_flag() {
    let trace_id = "t-reset";
    let mode = setup("reset", trace_id);
    detect_impl(
        &mode,
        trace_id,
        &commune_payload("Test"),
        &default_communes_params(),
    )
    .expect("détection préalable");
    assert!(file_path(&mode, trace_id).exists());

    reset_impl(&mode, trace_id).expect("la réinitialisation doit aboutir");

    assert!(
        !file_path(&mode, trace_id).exists(),
        "la description doit être supprimée"
    );
    assert!(
        !load_registry(&get_traces_path(&mode))[0].communes_completed,
        "le drapeau doit être reposé"
    );

    let _ = fs::remove_dir_all(&mode);
}

/// Une trace inconnue est refusée, et rien n'est touché.
#[test]
fn reset_rejects_an_unknown_trace() {
    let mode = test_mode_dir("reset_unknown");
    fs::create_dir_all(&mode).expect("dossier temporaire");

    let err = reset_impl(&mode, "absent").expect_err("une trace absente doit être refusée");

    assert!(err.contains("introuvable"), "message inattendu : {err}");
    let _ = fs::remove_dir_all(&mode);
}

// ─── Trace réécrite pendant l'analyse (volet de sûreté) ───────────────

/// Une trace dont le GPX a changé pendant l'analyse — un audit appliqué a
/// réécrit la géométrie — n'est **pas** marquée détectée : la description
/// calculée décrit l'ancienne trace. Rien n'est écrit, la détection sera rejouée.
#[test]
fn detection_is_abandoned_when_the_trace_changed_during_the_analysis() {
    let trace_id = "t-abandon";
    let mode = setup("abandon", trace_id);

    // Empreinte témoin différente de celle du registre : la trace a été réécrite.
    let abandonne = commit_detection(&mode, trace_id, &[], Some("sha256:empreinte-precedente"), false)
        .expect("l'appel doit aboutir");

    assert!(abandonne, "l'empreinte ne correspond plus : détection abandonnée");
    assert!(
        !file_path(&mode, trace_id).exists(),
        "aucune description ne doit être écrite"
    );
    assert!(
        !load_registry(&get_traces_path(&mode))[0].communes_completed,
        "le drapeau reste à false : la trace sera reprise"
    );

    // Empreinte conforme : la description est écrite et le drapeau posé.
    let empreinte = load_registry(&get_traces_path(&mode))[0].hash.clone();
    let abandonne = commit_detection(&mode, trace_id, &[], Some(&empreinte), false)
        .expect("l'appel doit aboutir");

    assert!(!abandonne);
    assert!(file_path(&mode, trace_id).exists(), "la description est écrite");
    assert!(load_registry(&get_traces_path(&mode))[0].communes_completed);

    let _ = fs::remove_dir_all(&mode);
}

// ─── Détection par hermes (partie locale) ─────────────────────────────

/// Réponse hermes minimale, relue par le type du client — c'est exactement ce
/// que la commande reçoit du serveur après le `POST /communes/detect`.
fn hermes_response(trace_id: &str, dataset_id: &str) -> HermesCommunesDetectResponse {
    serde_json::from_value(json!({
        "traceId": trace_id,
        "datasetId": dataset_id,
        "detectedAt": "2026-11-25T10:00:00Z",
        "passages": [
            {"commune": "Nice", "passage": 1, "nbPassagesTotal": 1,
             "kmEntree": 0.012_34, "kmSortie": 5.130_6, "longueurKm": 5.118_26,
             "ambiguous": false},
            {"commune": "Monaco", "passage": 1, "nbPassagesTotal": 1,
             "kmEntree": 5.130_6, "kmSortie": 5.4, "longueurKm": 0.269_4,
             "ambiguous": true}
        ]
    }))
    .expect("réponse hermes lisible")
}

/// La chaîne locale d'une détection hermes écrit le fichier au format du module
/// et pose le drapeau du registre.
#[test]
fn hermes_detection_writes_the_description_and_raises_the_flag() {
    let trace_id = "t-hermes";
    let mode = setup("hermes_detect", trace_id);
    let empreinte = load_registry(&get_traces_path(&mode))[0].hash.clone();
    let response = hermes_response(trace_id, "provence-alpes-cote-dazur");

    let detection = commit_hermes_detection(&mode, trace_id, &response, false, Some(&empreinte), 0.0, 5.4)
        .expect("la détection doit aboutir");

    assert_eq!(detection.dataset_id, "provence-alpes-cote-dazur");
    assert!(!detection.outside_coverage);
    assert_eq!(detection.passages.len(), 2);

    // Arrondi au mètre et invariant du fichier : longueur = sortie − entrée.
    let nice = &detection.passages[0];
    assert_eq!(nice.km_entree, 0.012);
    assert_eq!(nice.km_sortie, 5.131);
    assert_eq!(nice.longueur_km, 5.119);
    assert!(!nice.ambiguous);
    assert!(detection.passages[1].ambiguous, "le drapeau du serveur est conservé");

    assert_eq!(
        load_file(&mode, trace_id),
        Some(detection.passages.clone()),
        "la description est relisible telle quelle"
    );
    assert!(
        load_registry(&get_traces_path(&mode))[0].communes_completed,
        "le drapeau doit être posé"
    );

    let _ = fs::remove_dir_all(&mode);
}

/// Une trace réécrite pendant l'appel au serveur — un audit appliqué a changé la
/// géométrie — n'est pas marquée détectée : rien n'est écrit.
#[test]
fn hermes_detection_is_abandoned_when_the_trace_changed() {
    let trace_id = "t-hermes-abandon";
    let mode = setup("hermes_abandon", trace_id);
    let response = hermes_response(trace_id, "ds");

    let erreur = commit_hermes_detection(
        &mode,
        trace_id,
        &response,
        false,
        Some("sha256:empreinte-precedente"),
        0.0,
        5.4,
    )
    .expect_err("l'empreinte ne correspond plus");

    assert!(erreur.contains("abandonnée"), "message inattendu : {erreur}");
    assert!(
        !file_path(&mode, trace_id).exists(),
        "aucune description ne doit être écrite"
    );
    assert!(!load_registry(&get_traces_path(&mode))[0].communes_completed);

    let _ = fs::remove_dir_all(&mode);
}

/// Réponse vide : le serveur n'a trouvé aucun passage — trace hors couverture ou
/// réellement sans commune, le client ne le sait pas encore.
fn hermes_empty_response(trace_id: &str, dataset_id: &str) -> HermesCommunesDetectResponse {
    serde_json::from_value(json!({
        "traceId": trace_id,
        "datasetId": dataset_id,
        "detectedAt": "2026-11-25T10:00:00Z",
        "passages": []
    }))
    .expect("réponse hermes vide lisible")
}

/// Une **première** détection hors couverture enregistre le vide : la trace est
/// marquée détectée, et le verdict `outsideCoverage` remonte au front.
#[test]
fn hermes_first_detection_outside_coverage_is_recorded() {
    let trace_id = "t-hermes-outside";
    let mode = setup("hermes_outside", trace_id);
    let empreinte = load_registry(&get_traces_path(&mode))[0].hash.clone();
    let response = hermes_empty_response(trace_id, "test-monaco-ds");

    let detection =
        commit_hermes_detection(&mode, trace_id, &response, true, Some(&empreinte), 0.0, 10.0)
            .expect("une première détection vide est un résultat");

    assert!(detection.outside_coverage);
    assert!(detection.passages.is_empty());
    assert_eq!(detection.dataset_id, "test-monaco-ds");
    assert_eq!(detection.status, "partial", "aucun km couvert : détection partielle");
    assert_eq!(load_file(&mode, trace_id), Some(Vec::new()));
    assert!(
        !load_registry(&get_traces_path(&mode))[0].communes_completed,
        "une détection partielle n'est pas terminée"
    );

    let _ = fs::remove_dir_all(&mode);
}

/// Un **recalcul** hors couverture ne remplace pas une description existante :
/// le dataset servi a changé, le résultat serait un faux négatif destructeur.
#[test]
fn hermes_recalculation_outside_coverage_keeps_the_existing_description() {
    let trace_id = "t-hermes-keep";
    let mode = setup("hermes_keep", trace_id);
    let empreinte = load_registry(&get_traces_path(&mode))[0].hash.clone();

    // Détection initiale avec un dataset couvrant la trace.
    let response = hermes_response(trace_id, "provence-alpes-cote-dazur");
    commit_hermes_detection(&mode, trace_id, &response, false, Some(&empreinte), 0.0, 5.4)
        .expect("détection initiale");
    let avant = load_file(&mode, trace_id).expect("description initiale");

    // Recalcul avec un dataset qui ne couvre pas la trace.
    let vide = hermes_empty_response(trace_id, "test-monaco-ds");
    let erreur = commit_hermes_detection(&mode, trace_id, &vide, true, Some(&empreinte), 0.0, 5.4)
        .expect_err("le recalcul hors couverture doit être refusé");

    assert!(erreur.contains("conservée"), "message inattendu : {erreur}");
    assert_eq!(
        load_file(&mode, trace_id),
        Some(avant),
        "la description précédente est intacte"
    );
    assert!(
        load_registry(&get_traces_path(&mode))[0].communes_completed,
        "la trace reste marquée détectée"
    );

    let _ = fs::remove_dir_all(&mode);
}

/// Les micro-passages de frontière sont absorbés avant l'écriture : le fichier
/// ne conserve pas les oscillations de quelques dizaines de mètres.
#[test]
fn hermes_detection_smooths_the_boundary_micro_passages() {
    let trace_id = "t-hermes-smooth";
    let mode = setup("hermes_smooth", trace_id);
    let empreinte = load_registry(&get_traces_path(&mode))[0].hash.clone();
    let response: HermesCommunesDetectResponse = serde_json::from_value(json!({
        "traceId": trace_id,
        "datasetId": "EB-Menton",
        "detectedAt": "2026-11-25T15:00:00Z",
        "passages": [
            {"commune": "Roquebrune-Cap-Martin", "passage": 1, "nbPassagesTotal": 3,
             "kmEntree": 1.877, "kmSortie": 8.764, "longueurKm": 6.887, "ambiguous": false},
            {"commune": "Monaco", "passage": 1, "nbPassagesTotal": 2,
             "kmEntree": 8.764, "kmSortie": 8.804, "longueurKm": 0.040, "ambiguous": false},
            {"commune": "Roquebrune-Cap-Martin", "passage": 2, "nbPassagesTotal": 3,
             "kmEntree": 8.804, "kmSortie": 9.431, "longueurKm": 0.627, "ambiguous": false},
            {"commune": "Monaco", "passage": 2, "nbPassagesTotal": 2,
             "kmEntree": 9.431, "kmSortie": 13.255, "longueurKm": 3.824, "ambiguous": false}
        ]
    }))
    .expect("réponse hermes lisible");

    let detection = commit_hermes_detection(&mode, trace_id, &response, false, Some(&empreinte), 0.5, 11.378)
        .expect("la détection doit aboutir");

    assert_eq!(detection.passages.len(), 2, "le Monaco de 40 m est absorbé");
    assert_eq!(detection.status, "complete", "la couverture est intégrale");
    let roquebrune = &detection.passages[0];
    assert_eq!(roquebrune.commune, "Roquebrune-Cap-Martin");
    assert_eq!((roquebrune.km_entree, roquebrune.km_sortie), (1.877, 9.431));
    assert_eq!(roquebrune.longueur_km, 7.554);
    assert_eq!((roquebrune.passage, roquebrune.nb_passages_total), (1, 1));
    let monaco = &detection.passages[1];
    assert_eq!((monaco.km_entree, monaco.km_sortie), (9.431, 13.255));

    assert_eq!(
        load_file(&mode, trace_id),
        Some(detection.passages.clone()),
        "le fichier porte la segmentation lissée"
    );
    assert!(load_registry(&get_traces_path(&mode))[0].communes_completed);

    let _ = fs::remove_dir_all(&mode);
}

/// Une détection dont les passages ne couvrent pas toute la trace est
/// **partielle** : le fichier est écrit, mais l'état n'est pas terminé.
#[test]
fn hermes_detection_marks_a_partial_coverage() {
    let trace_id = "t-hermes-partial";
    let mode = setup("hermes_partial", trace_id);
    let empreinte = load_registry(&get_traces_path(&mode))[0].hash.clone();
    let response = hermes_response(trace_id, "provence-alpes-cote-dazur");

    // Les passages couvrent 5,388 km ; la trace en fait 20.
    let detection =
        commit_hermes_detection(&mode, trace_id, &response, false, Some(&empreinte), 0.0, 20.0)
            .expect("la détection doit aboutir");

    assert_eq!(detection.status, "partial");
    assert!(file_path(&mode, trace_id).exists(), "la description partielle est écrite");
    let registry = load_registry(&get_traces_path(&mode));
    assert!(
        !registry[0].communes_completed,
        "une détection partielle n'est pas terminée"
    );
    assert_eq!(registry[0].communes_status.as_deref(), Some("partial"));

    let _ = fs::remove_dir_all(&mode);
}

/// La requête transmet les points et les deux réglages retenus — `adminLevel` et
/// `minSegmentKm` —, sans `gapMaxM` (défaut du serveur).
#[test]
fn detection_request_transmits_the_local_settings() {
    let mut params = default_communes_params();
    params.admin_level = "6".to_string();
    params.min_segment_km = 0.02;

    let request = build_detection_request(
        "t-1",
        "provence-alpes-cote-dazur",
        vec![[2.0, 45.0], [2.02, 45.0]],
        &params,
    );
    let out = serde_json::to_value(&request).expect("requête sérialisable");

    assert_eq!(out["traceId"], "t-1");
    assert_eq!(out["datasetId"], "provence-alpes-cote-dazur");
    assert_eq!(out["points"][0], json!([2.0, 45.0]));
    assert_eq!(out["params"]["adminLevel"], "6");
    assert_eq!(out["params"]["minSegmentKm"], 0.02);
    assert!(out["params"].get("gapMaxM").is_none());
}
