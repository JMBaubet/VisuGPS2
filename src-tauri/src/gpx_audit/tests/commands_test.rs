//! Tests du module `gpx_audit::commands` (Livrable 4 §6).
//!
//! Les commandes Tauri exigent un `AppHandle` : elles sont donc testées au
//! travers de leurs **implémentations internes** (`detection_impl`,
//! `validate_impl`, …) et des fonctions pures de `pipeline`.

use std::fs;
use std::path::PathBuf;

use crate::gpx_audit::commands::{
    audit_params_from_settings, default_audit_params, detection_impl, findings_summary,
    validate_impl,
};
use crate::gpx_audit::corrections::{apply_delete, apply_route, mark_fp, undo_correction};
use crate::gpx_audit::pipeline::{self, renumber_findings};
use crate::gpx_audit::types::{
    AuditPoint, AuditState, CorrectionType, Finding, FindingContext, FindingContextIds,
    FindingKind, FindingPart, FindingStatus, LatLon, PartRole,
};
use crate::import_gpx::{
    get_geojson_path, get_trace_gpx_path, get_traces_path, load_registry, save_registry, Point3D,
    TraceMetadata, TraceStats,
};
use crate::settings::get_toml_value_by_path;

// ─── Aides de test ────────────────────────────────────────────────────

/// Dossier temporaire isolé (nettoyé au préalable), propre à chaque test.
fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vg2_audit_cmd_{}_{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// Chemin d'un fichier de scénario GPX de `docs/audit/reference/test_files/`.
fn scenario_path(prefix: &str) -> PathBuf {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../docs/audit/reference/test_files"
    );
    fs::read_dir(dir)
        .expect("dossier de test GPX introuvable")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .map(|n| {
                    let name = n.to_string_lossy();
                    name.starts_with(prefix) && name.ends_with(".gpx")
                })
                .unwrap_or(false)
        })
        .unwrap_or_else(|| panic!("fichier de test introuvable : {prefix}*.gpx"))
}

/// Trace brute régulière de `n` points vers l'est (pas de 10 m).
fn make_trace_points(n: usize) -> Vec<AuditPoint> {
    (0..n)
        .map(|i| AuditPoint {
            id: i as u32,
            lat: 45.0,
            lon: 2.0 + (i as f64) * 0.0001,
            ele: None,
        })
        .collect()
}

/// Finding minimal : une seule partie couvrant `[s..=e]`.
fn make_finding(id: &str, kind: FindingKind, s: usize, e: usize) -> Finding {
    Finding {
        id: id.to_string(),
        kind,
        label: format!("{} : 1", if kind == FindingKind::Ar { "Aller-retour" } else { "Boucle giratoire" }),
        summary: String::new(),
        peak: (s + e) / 2,
        peak_id: (s + e) as u32 / 2,
        pairs: Vec::new(),
        pair_idx: Vec::new(),
        ecart: None,
        d1: None,
        d2: None,
        total_angle: None,
        turn_text: None,
        core_ids: Vec::new(),
        zone_ids: Vec::new(),
        ctx_ids: FindingContextIds {
            up: None,
            dn: None,
        },
        ctx: FindingContext {
            up: None,
            dn: None,
        },
        parts: vec![FindingPart {
            s,
            e,
            role: PartRole::Warn,
            text: String::new(),
        }],
        status: FindingStatus::Pending,
        correction: None,
        undo: None,
    }
}

/// Métadonnée minimale pour le registre d'un mode temporaire.
fn make_trace_metadata(id: &str, filename: &str) -> TraceMetadata {
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
            points_count: 2,
            duration_s: None,
        },
        hash: "sha256:test".to_string(),
        favorite: false,
        is_displayed: false,
        cleaning_status: "clean".to_string(),
        cleaning_phase: String::new(),
        audit_status: "needs_review".to_string(),
    }
}

/// Prépare un mode temporaire avec une trace : `{dir}/traces/{id}/{filename}`
/// et le registre `{dir}/traces.json`. Retourne `(mode_dir, trace_id, filename)`.
fn setup_mode_with_trace(dir_name: &str, trace_id: &str, prefix: &str) -> (PathBuf, String, String) {
    let mode_dir = test_dir(dir_name);
    let filename = "trace.gpx".to_string();
    let trace_dir = mode_dir.join("traces").join(trace_id);
    fs::create_dir_all(&trace_dir).expect("création du dossier de trace");
    fs::copy(
        scenario_path(prefix),
        get_trace_gpx_path(&mode_dir, trace_id, &filename),
    )
    .expect("copie du scénario GPX");
    save_registry(
        &get_traces_path(&mode_dir),
        &[make_trace_metadata(trace_id, &filename)],
    )
    .expect("écriture du registre");
    (mode_dir, trace_id.to_string(), filename)
}

// ─── 1. Détection ─────────────────────────────────────────────────────

#[test]
fn test_run_detection_returns_valid_result() {
    let gpx = scenario_path("scenario_17_1");
    let result = detection_impl(&gpx, "t-1", &default_audit_params())
        .expect("la détection doit aboutir sur le scénario 17.1");

    assert_eq!(result.trace_id, "t-1");
    // Le scénario 17.1 est documenté : 27 points bruts → 23 après consolidation.
    assert_eq!(result.points.len(), 23);
    assert!(result.total_distance_m > 0.0, "longueur totale nulle");
    assert!(
        result.findings.iter().any(|f| f.kind == FindingKind::Ar),
        "le finding AR attendu est absent"
    );
    // Identifiants stables séquentiels 1..N.
    assert_eq!(result.points[0].id, 1);
    assert_eq!(result.points[22].id, 23);
    // Les findings publiés portent un identifiant de famille.
    assert!(result.findings.iter().all(|f| f.id.starts_with("ar-")
        || f.id.starts_with("rp-")));
    assert!(result.params.consol_m == default_audit_params().consol_m);
}

#[test]
fn test_run_detection_rejects_gpx_introuvable() {
    let missing = std::env::temp_dir().join("vg2_audit_absent.gpx");
    let err = detection_impl(&missing, "t-1", &default_audit_params())
        .expect_err("un GPX absent doit être refusé");
    assert!(err.contains("introuvable"), "message inattendu : {err}");
}

#[test]
fn test_run_detection_rejects_short_trace() {
    let dir = test_dir("short");
    fs::create_dir_all(&dir).expect("création du dossier temporaire");
    let gpx_path = dir.join("short.gpx");
    let xml = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<gpx version=\"1.1\" creator=\"Test\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n\
  <trk><name>Courte</name><trkseg>\n\
    <trkpt lat=\"45.0\" lon=\"2.0\"></trkpt>\n\
    <trkpt lat=\"45.0001\" lon=\"2.0\"></trkpt>\n\
    <trkpt lat=\"45.0002\" lon=\"2.0\"></trkpt>\n\
  </trkseg></trk>\n</gpx>\n";
    fs::write(&gpx_path, xml).expect("écriture du GPX court");

    let err = detection_impl(&gpx_path, "t-1", &default_audit_params())
        .expect_err("une trace de 3 points doit être refusée");
    assert_eq!(err, "Trace trop courte après consolidation.");
}

// ─── 2. Suppression ───────────────────────────────────────────────────

#[test]
fn test_apply_delete_rejects_nesting() {
    let points = make_trace_points(20);
    let findings = vec![
        make_finding("ar-1", FindingKind::Ar, 5, 10),
        make_finding("ar-2", FindingKind::Ar, 8, 12),
    ];

    // La plage [4..=11] englobe l'emprise du finding ar-2, resté pendant.
    let err = apply_delete(points, findings, "ar-1", 4, 11, 100)
        .expect_err("une suppression imbriquée doit être refusée");
    assert!(err.contains("ar-2"), "message inattendu : {err}");
}

#[test]
fn test_apply_delete_rejects_degenerate_trace() {
    let points = make_trace_points(5);
    let findings = vec![make_finding("ar-1", FindingKind::Ar, 0, 3)];
    let err = apply_delete(points, findings, "ar-1", 0, 3, 100)
        .expect_err("la trace ne doit pas descendre sous 2 points");
    assert!(err.contains("dégénérée"), "message inattendu : {err}");
}

#[test]
fn test_apply_delete_rejects_unknown_finding() {
    let points = make_trace_points(20);
    let findings = vec![make_finding("ar-1", FindingKind::Ar, 5, 10)];
    let err = apply_delete(points, findings, "ar-9", 5, 6, 100)
        .expect_err("un finding inconnu doit être refusé");
    assert!(err.contains("ar-9"), "message inattendu : {err}");
}

// ─── 3. Routage ───────────────────────────────────────────────────────

#[test]
fn test_apply_route_validates_profile() {
    let points = make_trace_points(20);
    let findings = vec![make_finding("ar-1", FindingKind::Ar, 5, 10)];
    let coords = vec![
        LatLon {
            lat: 45.0,
            lon: 2.0,
        },
        LatLon {
            lat: 45.0,
            lon: 2.1,
        },
    ];
    let err = apply_route(points, findings, "ar-1", 5, 8, coords, "unknown", 100)
        .expect_err("un profil inconnu doit être refusé");
    assert!(err.contains("unknown"), "message inattendu : {err}");
}

#[test]
fn test_apply_route_rejects_incomplete_tracé() {
    let points = make_trace_points(20);
    let findings = vec![make_finding("ar-1", FindingKind::Ar, 5, 10)];
    let coords = vec![LatLon {
        lat: 45.0,
        lon: 2.0,
    }];
    let err = apply_route(
        points,
        findings,
        "ar-1",
        5,
        8,
        coords,
        "driving-car",
        100,
    )
    .expect_err("un tracé d'un seul point doit être refusé");
    assert!(err.contains("2 points"), "message inattendu : {err}");
}

/// Point de vigilance 4 du Livrable 4 : un tracé ORS réduit à ses deux ancres
/// vide l'intérieur `[start+1 .. end-1]` — les ancres deviennent adjacentes.
#[test]
fn test_apply_route_with_anchor_only_coords_empties_interior() {
    let points = make_trace_points(20);
    let findings = vec![make_finding("ar-1", FindingKind::Ar, 5, 10)];
    let coords = vec![
        LatLon {
            lat: points[3].lat,
            lon: points[3].lon,
        },
        LatLon {
            lat: points[6].lat,
            lon: points[6].lon,
        },
    ];

    let state = apply_route(points, findings, "ar-1", 3, 6, coords, "driving-car", 100)
        .expect("un tracé à deux ancres doit être accepté");
    // Aucun point inséré : les 2 points d'intérieur sont retirés.
    assert_eq!(state.points.len(), 18, "aucun point ne doit être inséré");
    assert_eq!(state.points[3].id, 3, "l'ancre amont doit être conservée");
    assert_eq!(
        state.points[4].id, 6,
        "l'ancre aval doit suivre immédiatement"
    );
    let f = state
        .findings
        .iter()
        .find(|f| f.id == "ar-1")
        .expect("finding ar-1 présent");
    assert_eq!(f.status, FindingStatus::Corrected);
    assert_eq!(f.correction, Some(CorrectionType::RouteCar));
}

// ─── 4. Faux positif ──────────────────────────────────────────────────

#[test]
fn test_mark_fp_rejects_corrected() {
    let mut findings = vec![make_finding("ar-1", FindingKind::Ar, 5, 10)];
    findings[0].status = FindingStatus::Corrected;
    findings[0].correction = Some(CorrectionType::Delete);

    let err = mark_fp(findings, "ar-1").expect_err("un finding corrigé doit être refusé");
    assert!(!err.is_empty());
}

#[test]
fn test_mark_fp_then_unmark_returns_to_pending() {
    let findings = vec![make_finding("ar-1", FindingKind::Ar, 5, 10)];
    let marked = mark_fp(findings, "ar-1").expect("le marquage doit aboutir");
    assert_eq!(marked[0].status, FindingStatus::Fp);

    let unmarked =
        crate::gpx_audit::corrections::unmark_fp(marked, "ar-1").expect("le retrait doit aboutir");
    assert_eq!(unmarked[0].status, FindingStatus::Pending);
}

// ─── 5. Annulation ────────────────────────────────────────────────────

#[test]
fn test_undo_restores_state() {
    let points = make_trace_points(20);
    let original_ids: Vec<u32> = points.iter().map(|p| p.id).collect();
    let findings = vec![make_finding("ar-1", FindingKind::Ar, 5, 10)];

    let after = apply_delete(points, findings, "ar-1", 6, 9, 100).expect("suppression acceptée");
    assert_eq!(after.points.len(), 16);

    let AuditState { points, findings } =
        undo_correction(after.points, after.findings, "ar-1").expect("annulation acceptée");
    assert_eq!(points.len(), 20);
    assert_eq!(
        points.iter().map(|p| p.id).collect::<Vec<u32>>(),
        original_ids,
        "l'annulation doit restaurer la trace à l'identique"
    );
    assert_eq!(findings[0].status, FindingStatus::Pending);
    assert_eq!(findings[0].correction, None);
}

// ─── 6. Validation ────────────────────────────────────────────────────

#[test]
fn test_validate_rejects_pending() {
    let mode_dir = std::env::temp_dir().join("vg2_audit_validate_pending_inexistant");
    let points = make_trace_points(20);
    let findings = vec![make_finding("ar-1", FindingKind::Ar, 5, 10)];

    let err = validate_impl(&mode_dir, "t-1", &points, &findings, "")
        .expect_err("un finding pendant doit bloquer la validation");
    assert!(err.contains("1 restante"), "message inattendu : {err}");
}

#[test]
fn test_validate_rejects_degenerate_trace() {
    let mode_dir = std::env::temp_dir().join("vg2_audit_validate_degenere_inexistant");
    let err = validate_impl(&mode_dir, "t-1", &make_trace_points(1), &[], "")
        .expect_err("une trace d'un point doit être refusée");
    assert!(err.contains("dégénérée"), "message inattendu : {err}");
}

#[test]
fn test_validate_updates_audit_status() {
    let trace_id = "t-validate";
    let (mode_dir, _, filename) = setup_mode_with_trace("validate_ok", trace_id, "scenario_17_1");

    let points = make_trace_points(20);
    let mut findings = vec![make_finding("ar-1", FindingKind::Ar, 5, 10)];
    findings[0].status = FindingStatus::Fp;

    let updated = validate_impl(&mode_dir, trace_id, &points, &findings, "VérificationGPX")
        .expect("la validation doit aboutir");
    assert_eq!(updated.audit_status, "clean");

    // Le registre relu porte le nouveau statut, l'ancien module est intact.
    let registry = load_registry(&get_traces_path(&mode_dir));
    assert_eq!(registry[0].audit_status, "clean");
    assert_eq!(registry[0].cleaning_status, "clean");

    // Le GPX a été réécrit avec les points de travail et son bloc d'audit.
    let gpx_path = get_trace_gpx_path(&mode_dir, trace_id, &filename);
    let content = fs::read_to_string(&gpx_path).expect("lecture du GPX réécrit");
    assert!(content.contains("<audit "), "bloc d'audit absent");
    assert_eq!(
        content.matches("<trkpt ").count(),
        20,
        "le GPX doit porter la trace de travail"
    );

    // Backup posé une seule fois, jamais écrasé.
    let orig = gpx_path.with_extension("gpx.orig");
    assert!(orig.exists(), "le backup .orig doit exister");
    let orig_content = fs::read_to_string(&orig).expect("lecture du backup");
    assert_eq!(
        orig_content.matches("<trkpt ").count(),
        27,
        "le backup doit contenir le GPX d'origine"
    );

    // Dérivés régénérés.
    assert!(get_geojson_path(&mode_dir, trace_id).exists());
}

#[test]
fn test_validate_rejects_unknown_trace() {
    let mode_dir = test_dir("validate_unknown");
    fs::create_dir_all(&mode_dir).expect("création du dossier temporaire");
    let err = validate_impl(&mode_dir, "absent", &make_trace_points(20), &[], "")
        .expect_err("une trace absente du registre doit être refusée");
    assert!(err.contains("introuvable"), "message inattendu : {err}");
}

// ─── 7. Pipeline ──────────────────────────────────────────────────────

#[test]
fn test_renumber_findings_par_famille() {
    let mut findings = vec![
        make_finding("ar-9", FindingKind::Ar, 10, 12),
        make_finding("rp-4", FindingKind::Rp, 20, 24),
        make_finding("ar-7", FindingKind::Ar, 30, 33),
    ];
    findings[0].label = "Aller-retour : 9".to_string();
    findings[1].label = "Tour de rond-point : 4".to_string();
    findings[2].label = "Aller-retour : 7".to_string();

    renumber_findings(&mut findings);

    assert_eq!(findings[0].id, "ar-1");
    assert_eq!(findings[0].label, "Aller-retour : 1");
    assert_eq!(findings[1].id, "rp-1");
    assert_eq!(findings[1].label, "Tour de rond-point : 1");
    assert_eq!(findings[2].id, "ar-2");
    assert_eq!(findings[2].label, "Aller-retour : 2");

    // Idempotence : une seconde passe ne change rien.
    let snapshot: Vec<(String, String)> = findings
        .iter()
        .map(|f| (f.id.clone(), f.label.clone()))
        .collect();
    renumber_findings(&mut findings);
    let again: Vec<(String, String)> = findings
        .iter()
        .map(|f| (f.id.clone(), f.label.clone()))
        .collect();
    assert_eq!(snapshot, again);
}

#[test]
fn test_read_source_context_extracts_header() {
    let ctx = pipeline::read_source_context(&scenario_path("scenario_17_1"))
        .expect("le contexte source doit être lisible");
    assert!(
        ctx.attrs
            .as_deref()
            .map_or(false, |a| a.contains("creator=")),
        "attributs <gpx> non extraits : {:?}",
        ctx.attrs
    );
    assert!(
        ctx.meta_xml
            .as_deref()
            .map_or(false, |m| m.starts_with("<metadata")),
        "élément <metadata> non extrait : {:?}",
        ctx.meta_xml
    );
    assert_eq!(ctx.trk_name.as_deref(), Some("AR 17.1"));
}

#[test]
fn test_findings_summary_counts() {
    let mut findings = vec![
        make_finding("ar-1", FindingKind::Ar, 0, 2),
        make_finding("ar-2", FindingKind::Ar, 4, 6),
        make_finding("rp-1", FindingKind::Rp, 8, 12),
    ];
    findings[0].status = FindingStatus::Corrected;
    findings[0].correction = Some(CorrectionType::RouteCar);
    findings[1].status = FindingStatus::Corrected;
    findings[1].correction = Some(CorrectionType::Delete);
    findings[2].status = FindingStatus::Fp;

    let summary = findings_summary(&findings);
    assert_eq!(summary.routes, 1);
    assert_eq!(summary.deletions, 1);
    assert_eq!(summary.false_positives, 1);
}

/// Les 8 clés `Audit.*` lues par le code doivent exister dans le TOML réel :
/// c'est le piège n°3 du Livrable 6 (un nom de groupe mal orthographié — point
/// contre tiret — passerait inaperçu, le repli masquant l'erreur).
#[test]
fn test_audit_settings_keys_exist_in_default_toml() {
    let raw = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/settings.default.toml"
    ))
    .expect("settings.default.toml doit être lisible");
    let table: toml::Table =
        toml::from_str(&raw).expect("settings.default.toml doit être du TOML valide");

    for path in [
        "Audit.Consolidation.seuil",
        "Audit.AR.toleranceDeg",
        "Audit.AR.seuilPaireM",
        "Audit.AR.maxPaires",
        "Audit.AR.branchesMaxM",
        "Audit.RP.seuilFermetureM",
        "Audit.RP.angleMinDeg",
        "Audit.Application.nom",
        "Audit.OpenRouteService.clePrimaire",
        "Audit.OpenRouteService.cleSecondaire",
    ] {
        assert!(
            get_toml_value_by_path(&table, path).is_some(),
            "paramètre absent de settings.default.toml : {path}"
        );
    }

    // Les valeurs publiées sont bien celles du TOML (et non le repli, qui leur
    // est identique par construction).
    let params = audit_params_from_settings(&table, &toml::Table::new());
    let expected = default_audit_params();
    assert_eq!(params.consol_m, expected.consol_m);
    assert_eq!(params.tol_deg, expected.tol_deg);
    assert_eq!(params.pair_m, expected.pair_m);
    assert_eq!(params.maxpairs, expected.maxpairs);
    assert_eq!(params.seg_m, expected.seg_m);
    assert_eq!(params.close_m, expected.close_m);
    assert_eq!(params.angle_deg, expected.angle_deg);

    // La surcharge utilisateur est prioritaire.
    let mut overrides = toml::Table::new();
    overrides.insert("Audit".to_string(), {
        let mut ar = toml::Table::new();
        ar.insert("toleranceDeg".to_string(), toml::Value::Float(33.0));
        let mut audit = toml::Table::new();
        audit.insert("AR".to_string(), toml::Value::Table(ar));
        toml::Value::Table(audit)
    });
    let overridden = audit_params_from_settings(&table, &overrides);
    assert_eq!(overridden.tol_deg, 33.0);
}

#[test]
fn test_write_atomic_replaces_content() {
    let dir = test_dir("atomic");
    fs::create_dir_all(&dir).expect("création du dossier temporaire");
    let path = dir.join("cible.json");

    pipeline::write_atomic(&path, b"premier").expect("première écriture");
    assert_eq!(fs::read_to_string(&path).unwrap(), "premier");

    pipeline::write_atomic(&path, b"second").expect("seconde écriture");
    assert_eq!(fs::read_to_string(&path).unwrap(), "second");

    // Aucun fichier temporaire résiduel.
    let leftovers: Vec<String> = fs::read_dir(&dir)
        .expect("lecture du dossier")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "temporaires résiduels : {leftovers:?}");
}
