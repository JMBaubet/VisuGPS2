//! Tests des commandes du module Multiride.
//!
//! Les commandes Tauri exigent un `AppHandle` : elles sont testées au travers
//! de leurs **implémentations internes** (`detect_impl`, `load_impl`,
//! `validate_impl`), qui ne dépendent que du dossier du mode d'exécution — même
//! approche que `gpx_audit::tests::commands_test`.

use std::fs;
use std::path::PathBuf;

use crate::gpx_multiride::commands::{
    default_multiride_params, detect_impl, detect_status, load_impl, multiride_params_from_settings,
    toggle_fp_impl, undo_impl, validate_impl,
};
use crate::gpx_multiride::file::{build_archive, file_path, load_file, save_file};
use crate::gpx_multiride::types::{
    MultirideArchive, MultirideLatLon, MultirideParams, MultiridePassage, MultirideSens,
    STATUS_NONE, STATUS_VALIDATED,
};
use crate::import_gpx::{
    get_trace_gpx_path, get_traces_path, load_registry, save_registry, Point3D, TraceMetadata,
    TraceStats,
};
use crate::settings::get_toml_value_by_path;

// ─── Aides de test ────────────────────────────────────────────────────

/// Dossier temporaire isolé (nettoyé au préalable), propre à chaque test.
fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vg2_multiride_cmd_{}_{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn params() -> MultirideParams {
    MultirideParams {
        tolerance_m: 10.0,
        longueur_min_m: 100.0,
        pas_echantillonnage_m: 4.0,
        fusion_references_m: 100.0,
    }
}

/// GPX minimal de `n` points alignés vers l'est (~11 m entre deux points).
fn make_gpx(n: usize) -> String {
    let mut body = String::new();
    for i in 0..n {
        body.push_str(&format!(
            "<trkpt lat=\"{:.6}\" lon=\"{:.6}\"><ele>120.0</ele></trkpt>",
            45.0 + (i as f64) * 0.0001,
            2.0 + (i as f64) * 0.0001
        ));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <gpx version=\"1.1\" creator=\"test\" xmlns=\"http://www.topografix.com/GPX/1/1\">\
         <trk><name>Trace de test</name><trkseg>{}</trkseg></trk></gpx>",
        body
    )
}

/// Métadonnée minimale du registre (trace auditée, sans détection de passages
/// multiples).
fn make_trace(id: &str, filename: &str) -> TraceMetadata {
    TraceMetadata {
        id: id.to_string(),
        name: "Trace de test".to_string(),
        source: "Test".to_string(),
        source_url: None,
        activity_type: None,
        filename: filename.to_string(),
        import_date: "2026-01-01T00:00:00Z".to_string(),
        stats: TraceStats {
            start_point: Point3D {
                lat: 45.0,
                lon: 2.0,
                alt: None,
            },
            end_point: Point3D {
                lat: 45.0,
                lon: 2.1,
                alt: None,
            },
            distance_m: 1000.0,
            positive_elevation_m: 0.0,
            negative_elevation_m: 0.0,
            alt_min_m: None,
            alt_max_m: None,
            points_count: 8,
            duration_s: None,
        },
        hash: "sha256:test".to_string(),
        favorite: false,
        is_displayed: false,
        audit_status: "clean".to_string(),
        audit_archived: false,
        multiride_status: None,
    }
}

/// Mode temporaire contenant une trace et son GPX, prêt pour la détection.
fn mode_with_trace(name: &str, gpx_points: usize) -> (PathBuf, String) {
    let mode = test_dir(name);
    let id = "trace-multiride-1".to_string();
    let filename = "trace_test.gpx".to_string();

    fs::create_dir_all(mode.join("traces").join(&id)).unwrap();
    fs::write(
        get_trace_gpx_path(&mode, &id, &filename),
        make_gpx(gpx_points),
    )
    .unwrap();
    save_registry(&get_traces_path(&mode), &[make_trace(&id, &filename)]).unwrap();

    (mode, id)
}

/// Statut de la trace dans le registre du mode.
fn registry_status(mode: &std::path::Path, id: &str) -> Option<String> {
    load_registry(&get_traces_path(mode))
        .into_iter()
        .find(|t| t.id == id)
        .and_then(|t| t.multiride_status)
}

/// État portant un passage, pour les tests de validation.
fn archive_with_one_passage(mode: &std::path::Path, id: &str) -> MultirideArchive {
    let archive = build_archive(
        id,
        "trace_test.gpx",
        params(),
        8,
        0.08,
        false,
        vec![MultiridePassage {
            segment: 1,
            passage: 1,
            sens: MultirideSens::Reference,
            faux_positif: false,
            point_entree: 1,
            point_sortie: 8,
            km_entree: 0.0,
            km_sortie: 0.08,
            longueur_km: 0.08,
            fusionne: false,
            valide: false,
            avant_fusion: None,
            entree: MultirideLatLon {
                lat: 45.0,
                lon: 2.0,
            },
            sortie: MultirideLatLon {
                lat: 45.0007,
                lon: 2.0007,
            },
        }],
    );
    save_file(&file_path(mode, id), &archive).unwrap();
    archive
}

// ─── Détection ────────────────────────────────────────────────────────

/// La détection écrit le fichier de description, pose le statut dans le
/// registre et retourne l'état — le tout sans aucune trace de passages pour une
/// trace sans répétition.
#[test]
fn detect_writes_the_file_and_marks_the_trace() {
    let (mode, id) = mode_with_trace("detect", 8);

    let result = detect_impl(&mode, &id, params()).unwrap();

    assert_eq!(result.status, STATUS_NONE);
    assert!(result.archive.passages.is_empty());
    assert_eq!(result.archive.trace_id, id);
    assert_eq!(result.archive.source, "trace_test.gpx");
    assert_eq!(result.archive.trace_point_count, 8);
    // 7 intervalles de ~11 m (0,0001° de latitude et de longitude).
    assert!(
        result.archive.trace_length_km > 0.05 && result.archive.trace_length_km < 0.2,
        "longueur inattendue : {} km",
        result.archive.trace_length_km
    );
    assert!(result.archive.updated_at.ends_with('Z'));

    let path = file_path(&mode, &id);
    assert!(path.exists(), "le fichier de description doit être écrit");
    assert!(load_file(&path, &id).is_some(), "fichier écrit mais illisible");
    assert_eq!(registry_status(&mode, &id).as_deref(), Some(STATUS_NONE));
}

/// La relecture rend exactement l'état écrit : le fichier est la persistance du
/// module, il n'y a pas d'état côté Rust.
#[test]
fn load_returns_what_detection_wrote() {
    let (mode, id) = mode_with_trace("load", 8);
    let detected = detect_impl(&mode, &id, params()).unwrap();

    let loaded = load_impl(&mode, &id).expect("fichier écrit mais non relu");

    assert_eq!(loaded.trace_id, detected.archive.trace_id);
    assert_eq!(loaded.updated_at, detected.archive.updated_at);
    assert_eq!(loaded.trace_point_count, detected.archive.trace_point_count);
    // Comparaison à la tolérance : la relecture d'un flottant JSON n'est pas
    // garantie bit à bit par `serde_json` (1 ulp d'écart possible sur une
    // valeur calculée, sans le feature `float_roundtrip`).
    assert!(
        (loaded.trace_length_km - detected.archive.trace_length_km).abs() < 1e-9,
        "longueur relue {} ≠ calculée {}",
        loaded.trace_length_km,
        detected.archive.trace_length_km
    );
    assert_eq!(loaded.params.tolerance_m, detected.archive.params.tolerance_m);
}

/// Sans fichier, la relecture est silencieuse (`None`) : l'appelant relance la
/// détection, qui reste la source de vérité.
#[test]
fn load_returns_none_without_a_file() {
    let (mode, id) = mode_with_trace("load_absent", 8);
    assert!(load_impl(&mode, &id).is_none());
}

/// Une trace absente du registre est une incohérence d'appel : elle est
/// remontée, jamais silencieuse.
#[test]
fn detect_fails_on_unknown_trace() {
    let (mode, _) = mode_with_trace("detect_inconnue", 8);
    let error = detect_impl(&mode, "trace-inexistante", params()).unwrap_err();
    assert!(error.contains("Trace introuvable"), "message inattendu : {error}");
}

/// Une trace dégénérée (moins de 2 points) est refusée avant toute écriture.
#[test]
fn detect_fails_on_degenerate_trace() {
    let (mode, id) = mode_with_trace("detect_degeneree", 1);
    let error = detect_impl(&mode, &id, params()).unwrap_err();
    assert!(error.contains("dégénérée"), "message inattendu : {error}");
    assert!(
        registry_status(&mode, &id).is_none(),
        "un échec ne doit rien écrire dans le registre"
    );
    assert!(!file_path(&mode, &id).exists());
}

// ─── Validation ───────────────────────────────────────────────────────

/// Valider marque l'état, réécrit le fichier et pose `validated` dans le
/// registre : c'est ce statut qui ouvre l'édition caméra.
#[test]
fn validate_marks_the_state_and_opens_the_barrier() {
    let (mode, id) = mode_with_trace("validate", 8);
    detect_impl(&mode, &id, params()).unwrap();
    let archive = archive_with_one_passage(&mode, &id);

    let updated = validate_impl(&mode, &id, archive).unwrap();

    assert!(updated.valide);
    assert_eq!(updated.status(), STATUS_VALIDATED);
    assert_eq!(registry_status(&mode, &id).as_deref(), Some(STATUS_VALIDATED));

    let reloaded = load_file(&file_path(&mode, &id), &id).unwrap();
    assert!(reloaded.valide);
    assert_eq!(reloaded.passages.len(), 1);
}

/// Un ajustement postérieur à la validation réécrit le fichier **sans** faire
/// rebasculer le statut : les ajustements n'ont pas d'incidence sur l'édition
/// caméra.
#[test]
fn adjustment_after_validation_keeps_the_status() {
    let (mode, id) = mode_with_trace("validate_ajustement", 8);
    detect_impl(&mode, &id, params()).unwrap();
    let archive = archive_with_one_passage(&mode, &id);
    let validated = validate_impl(&mode, &id, archive).unwrap();

    // Ajustement : le passage est marqué faux positif. Le store ajuste l'état
    // **validé** qu'il a reçu, donc `valide` est conservé à la réécriture.
    let mut adjusted = validated;
    adjusted.passages[0].faux_positif = true;
    save_file(&file_path(&mode, &id), &adjusted).unwrap();

    let reloaded = load_file(&file_path(&mode, &id), &id).unwrap();
    assert!(reloaded.valide);
    assert!(reloaded.passages[0].faux_positif);
    assert_eq!(reloaded.status(), STATUS_VALIDATED);
    assert_eq!(registry_status(&mode, &id).as_deref(), Some(STATUS_VALIDATED));
}

/// Un ajustement, puis son annulation, laissent le registre intact : seul le
/// statut de validation décide de la barrière de l'édition caméra.
#[test]
fn an_adjustment_and_its_undo_leave_the_registry_alone() {
    let (mode, id) = mode_with_trace("undo_registre", 8);
    detect_impl(&mode, &id, params()).unwrap();
    let archive = archive_with_one_passage(&mode, &id);

    let marked = toggle_fp_impl(&mode, &id, archive, 1).unwrap();
    assert!(marked.passages[0].faux_positif);
    assert_eq!(registry_status(&mode, &id).as_deref(), Some(STATUS_NONE));

    let undone = undo_impl(&mode, &id, marked, 1).unwrap();

    assert!(!undone.passages[0].faux_positif);
    assert_eq!(
        registry_status(&mode, &id).as_deref(),
        Some(STATUS_NONE),
        "un ajustement n'a pas d'incidence sur la barrière"
    );
    let reloaded = load_file(&file_path(&mode, &id), &id).unwrap();
    assert!(!reloaded.passages[0].faux_positif, "l'annulation est écrite");
}

/// Un état appartenant à une autre trace est refusé : le rattachement du
/// fichier à sa trace est vérifié avant écriture.
#[test]
fn validate_rejects_a_foreign_archive() {
    let (mode, id) = mode_with_trace("validate_etrangere", 8);
    detect_impl(&mode, &id, params()).unwrap();
    let mut archive = archive_with_one_passage(&mode, &id);
    archive.trace_id = "autre-trace".to_string();

    let error = validate_impl(&mode, &id, archive).unwrap_err();

    assert!(error.contains("autre trace"), "message inattendu : {error}");
    assert_eq!(
        registry_status(&mode, &id).as_deref(),
        Some(STATUS_NONE),
        "un refus ne doit pas modifier le registre"
    );
}

// ─── Paramètres de détection ──────────────────────────────────────────

/// Table du schéma embarqué, telle que la lit le système de paramètres.
fn default_schema() -> toml::Table {
    let raw = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/settings.default.toml"
    ))
    .expect("settings.default.toml doit être lisible");
    toml::from_str(&raw).expect("settings.default.toml doit être du TOML valide")
}

/// Les clés `Multiride.Detection` existent dans le schéma, et les valeurs
/// publiées sont bien celles du TOML — un repli identique par construction ne
/// prouverait rien.
#[test]
fn the_detection_settings_exist_in_the_schema() {
    let table = default_schema();

    for path in [
        "Multiride.Detection.tolerance",
        "Multiride.Detection.longueurMin",
        "Multiride.Detection.pasEchantillonnage",
        "Multiride.Detection.fusionReferences",
    ] {
        assert!(
            get_toml_value_by_path(&table, path).is_some(),
            "paramètre absent de settings.default.toml : {path}"
        );
    }

    let params = multiride_params_from_settings(&table, &toml::Table::new());
    let expected = default_multiride_params();
    assert_eq!(params.tolerance_m, expected.tolerance_m);
    assert_eq!(params.longueur_min_m, expected.longueur_min_m);
    assert_eq!(params.pas_echantillonnage_m, expected.pas_echantillonnage_m);
    assert_eq!(params.fusion_references_m, expected.fusion_references_m);
}

/// La surcharge utilisateur prime sur le schéma, et les autres valeurs restent
/// celles du schéma.
#[test]
fn a_user_override_wins_over_the_schema() {
    let table = default_schema();
    let overrides: toml::Table = toml::from_str(
        "[Multiride.Detection]\ntolerance = 15\nlongueurMin = 250\n",
    )
    .expect("surcharge TOML de test");

    let params = multiride_params_from_settings(&table, &overrides);
    let expected = default_multiride_params();

    assert_eq!(params.tolerance_m, 15.0);
    assert_eq!(params.longueur_min_m, 250.0);
    assert_eq!(params.pas_echantillonnage_m, expected.pas_echantillonnage_m);
    assert_eq!(params.fusion_references_m, expected.fusion_references_m);
}

// ─── Chaîne automatique ───────────────────────────────────────────────

/// La détection d'une chaîne automatique écrit le fichier de description et
/// retourne le statut à poser dans le registre.
///
/// Elle ne touche pas au registre : c'est l'appelant — import ou validation
/// d'audit — qui pose le statut, dans la même passe que ses propres
/// modifications, pour qu'aucun état intermédiaire ne les oppose.
#[test]
fn detect_status_writes_the_description_file() {
    let (mode, id) = mode_with_trace("hook_ok", 8);
    let gpx_path = get_trace_gpx_path(&mode, &id, "trace_test.gpx");

    let status = detect_status(&mode, &id, "trace_test.gpx", &gpx_path, params());

    assert_eq!(status.as_deref(), Some(STATUS_NONE));
    assert!(
        file_path(&mode, &id).exists(),
        "le fichier de description doit être écrit"
    );
    assert_eq!(
        registry_status(&mode, &id),
        None,
        "la chaîne automatique laisse le registre à l'appelant"
    );
}

/// Une défaillance de la détection ne fait pas échouer l'opération qui l'a
/// déclenchée : elle répond `None`, et rien n'est écrit.
#[test]
fn detect_status_stays_silent_on_a_failure() {
    let (mode, id) = mode_with_trace("hook_ko", 8);
    let missing = mode.join("traces").join(&id).join("absent.gpx");

    let status = detect_status(&mode, &id, "absent.gpx", &missing, params());

    assert_eq!(status, None, "une GPX illisible ne doit pas remonter d'erreur");
    assert!(
        !file_path(&mode, &id).exists(),
        "un échec ne doit rien écrire"
    );
}
