//! Tests de `gpx_audit::archive` — persistance de l'état de travail.
//!
//! L'archive renverse la décision 6 (« findings volatils ») : elle est écrite au
//! fil des actions, reprise pour reprendre une session interrompue et relue pour
//! consulter un audit validé. Ces tests couvrent le contrat de fichier (chemin,
//! format camelCase, écrasement, écriture atomique) et la robustesse de la
//! lecture (absente, illisible, version inconnue, autre trace).

use std::fs;
use std::path::PathBuf;

use crate::gpx_audit::archive::{
    archive_path, build_archive, load_archive, save_archive, ARCHIVE_VERSION,
};
use crate::gpx_audit::commands::{default_audit_params, load_archive_impl, save_state_impl};
use crate::gpx_audit::types::{
    AuditPoint, CorrectionType, Finding, FindingContext, FindingContextIds, FindingKind,
    FindingPart, FindingStatus, PartRole, UndoDelete, UndoRecord,
};

// ─── Aides de test ────────────────────────────────────────────────────

/// Dossier temporaire isolé (nettoyé au préalable), propre à chaque test.
fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vg2_audit_archive_{}_{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// Trace de `n` points, identifiants séquentiels 1..N (comme la détection).
fn make_points(n: usize) -> Vec<AuditPoint> {
    (0..n)
        .map(|i| AuditPoint {
            id: (i + 1) as u32,
            lat: 45.0,
            lon: 2.0 + (i as f64) * 0.0001,
            ele: Some(120.0),
        })
        .collect()
}

/// Finding minimal portant un statut, une correction et un enregistrement
/// d'annulation éventuels.
fn make_finding(
    id: &str,
    status: FindingStatus,
    correction: Option<CorrectionType>,
    undo: Option<UndoRecord>,
) -> Finding {
    Finding {
        id: id.to_string(),
        kind: FindingKind::Ar,
        label: format!("Aller-retour : {id}"),
        summary: "1 paire".to_string(),
        peak: 2,
        peak_id: 3,
        pairs: Vec::new(),
        pair_idx: Vec::new(),
        ecart: Some(12.5),
        d1: Some(60.0),
        d2: Some(58.0),
        total_angle: None,
        turn_text: None,
        core_ids: Vec::new(),
        zone_ids: vec![2, 3],
        ctx_ids: FindingContextIds {
            up: Some(2),
            dn: Some(4),
        },
        ctx: FindingContext {
            up: Some(1),
            dn: Some(4),
        },
        parts: vec![FindingPart {
            s: 1,
            e: 3,
            role: PartRole::Warn,
            text: "zone".to_string(),
        }],
        status,
        correction,
        undo,
    }
}

// ─── Contrat de fichier ───────────────────────────────────────────────

#[test]
fn test_archive_path_follows_trace_folder() {
    let mode_dir = PathBuf::from("/tmp/vg2_mode");
    assert_eq!(
        archive_path(&mode_dir, "abc"),
        PathBuf::from("/tmp/vg2_mode/traces/abc/audit.json")
    );
}

#[test]
fn test_save_then_load_round_trip() {
    let mode_dir = test_dir("round_trip");
    let trace_id = "t-archive";
    let params = default_audit_params();
    let points = make_points(5);
    let findings = vec![
        make_finding(
            "ar-1",
            FindingStatus::Corrected,
            Some(CorrectionType::Delete),
            None,
        ),
        make_finding("ar-2", FindingStatus::Fp, None, None),
        make_finding("ar-3", FindingStatus::Pending, None, None),
    ];

    save_state_impl(&mode_dir, trace_id, params.clone(), points, findings, false)
        .expect("écriture de l'archive");

    let loaded = load_archive_impl(&mode_dir, trace_id).expect("archive relue");
    assert_eq!(loaded.version, ARCHIVE_VERSION);
    assert_eq!(loaded.trace_id, trace_id);
    assert!(!loaded.validated);
    assert!(!loaded.updated_at.is_empty(), "horodatage manquant");
    assert_eq!(loaded.points.len(), 5);
    assert_eq!(loaded.points[0].id, 1);
    assert_eq!(loaded.points[4].id, 5);
    assert_eq!(loaded.findings.len(), 3);
    assert_eq!(loaded.findings[0].status, FindingStatus::Corrected);
    assert_eq!(loaded.findings[0].correction, Some(CorrectionType::Delete));
    assert_eq!(loaded.findings[1].status, FindingStatus::Fp);
    assert_eq!(loaded.findings[2].status, FindingStatus::Pending);
    // Les paramètres voyagent avec l'archive : les aperçus et les éléments de
    // rendu de la carte sont reconstruits à l'identique en consultation.
    assert_eq!(loaded.params.consol_m, params.consol_m);
    assert_eq!(loaded.params.angle_deg, params.angle_deg);
}

#[test]
fn test_archive_keeps_deleted_points_of_a_correction() {
    // Les points supprimés sont conservés : ils sont la seule source de
    // « l'origine » de l'anomalie en consultation.
    let mode_dir = test_dir("undo_points");
    let trace_id = "t-undo";
    let removed = make_points(3);
    let undo = UndoRecord::Delete(UndoDelete {
        orig_pts: removed,
        anchor_left_id: Some(1),
        anchor_right_id: Some(2),
        first_no: 3,
        absorbed_fp: Vec::new(),
    });
    let findings = vec![make_finding(
        "ar-1",
        FindingStatus::Corrected,
        Some(CorrectionType::Delete),
        Some(undo),
    )];

    save_state_impl(
        &mode_dir,
        trace_id,
        default_audit_params(),
        make_points(2),
        findings,
        true,
    )
    .expect("écriture de l'archive");

    let loaded = load_archive_impl(&mode_dir, trace_id).expect("archive relue");
    assert!(loaded.validated, "l'archive validée doit le rester");
    let undo = loaded.findings[0]
        .undo
        .as_ref()
        .expect("enregistrement d'annulation conservé");
    match undo {
        UndoRecord::Delete(record) => {
            assert_eq!(record.orig_pts.len(), 3);
            assert_eq!(record.orig_pts[0].id, 1);
        }
        _ => panic!("nature d'annulation inattendue"),
    }
}

#[test]
fn test_save_overwrites_previous_archive() {
    // L'archive reflète toujours le dernier état écrit : pas d'historique de
    // versions, pas de fusion.
    let mode_dir = test_dir("overwrite");
    let trace_id = "t-ow";
    save_state_impl(
        &mode_dir,
        trace_id,
        default_audit_params(),
        make_points(4),
        vec![],
        false,
    )
    .expect("première écriture");
    save_state_impl(
        &mode_dir,
        trace_id,
        default_audit_params(),
        make_points(2),
        vec![],
        true,
    )
    .expect("seconde écriture");

    let loaded = load_archive_impl(&mode_dir, trace_id).expect("archive relue");
    assert_eq!(loaded.points.len(), 2, "la seconde écriture doit remplacer la première");
    assert!(loaded.validated);
}

#[test]
fn test_save_creates_trace_folder_without_temporary_file() {
    let mode_dir = test_dir("folder");
    let trace_id = "t-new";
    let archive = build_archive(
        trace_id,
        false,
        default_audit_params(),
        make_points(2),
        Vec::new(),
    );
    let path = archive_path(&mode_dir, trace_id);
    assert!(!path.exists(), "l'archive ne doit pas exister avant écriture");

    save_archive(&path, &archive).expect("écriture de l'archive");
    assert!(path.exists(), "archive absente après écriture");
    assert!(
        !path.with_file_name("audit.json.tmp").exists(),
        "fichier temporaire résiduel après écriture"
    );
}

#[test]
fn test_archive_json_uses_camel_case() {
    // Contrat avec le miroir TypeScript (`src/stores/audit.ts`) : les clés sont
    // en camelCase, comme les retours de commande.
    let mode_dir = test_dir("camel_case");
    let trace_id = "t-case";
    let findings = vec![make_finding("ar-1", FindingStatus::Pending, None, None)];
    save_state_impl(
        &mode_dir,
        trace_id,
        default_audit_params(),
        make_points(3),
        findings,
        false,
    )
    .expect("écriture de l'archive");

    let content = fs::read_to_string(archive_path(&mode_dir, trace_id)).expect("lecture brute");
    for key in [
        "\"version\"",
        "\"traceId\"",
        "\"updatedAt\"",
        "\"validated\"",
        "\"params\"",
        "\"points\"",
        "\"findings\"",
        "\"consolM\"",
        "\"peakId\"",
        "\"zoneIds\"",
        "\"status\"",
    ] {
        assert!(content.contains(key), "clé absente du JSON : {key}");
    }
    assert!(
        !content.contains("trace_id"),
        "les clés doivent être en camelCase (miroir TS)"
    );
}

// ─── Robustesse de la lecture ─────────────────────────────────────────

#[test]
fn test_load_returns_none_when_archive_is_absent() {
    let mode_dir = test_dir("absente");
    assert!(load_archive_impl(&mode_dir, "t-absente").is_none());
}

#[test]
fn test_load_returns_none_on_corrupted_file() {
    let mode_dir = test_dir("corrompue");
    let trace_id = "t-corrompue";
    let path = archive_path(&mode_dir, trace_id);
    fs::create_dir_all(path.parent().expect("dossier de trace")).expect("création du dossier");
    fs::write(&path, b"{ ceci n'est pas du JSON").expect("écriture du fichier corrompu");

    assert!(
        load_archive_impl(&mode_dir, trace_id).is_none(),
        "une archive illisible doit être ignorée (repli sur la détection)"
    );
}

#[test]
fn test_load_returns_none_on_unknown_version() {
    let mode_dir = test_dir("version");
    let trace_id = "t-version";
    let mut archive = build_archive(
        trace_id,
        false,
        default_audit_params(),
        make_points(2),
        Vec::new(),
    );
    archive.version = ARCHIVE_VERSION + 1;
    let path = archive_path(&mode_dir, trace_id);
    save_archive(&path, &archive).expect("écriture de l'archive");
    // La lecture directe par le chemin confirme que c'est bien la version qui
    // est refusée, et non l'absence de fichier.
    assert!(load_archive(&path, trace_id).is_none());
}

#[test]
fn test_load_returns_none_for_another_trace() {
    // Garde d'identité : une archive déplacée dans le dossier d'une autre trace
    // est ignorée (on préfère une détection fraîche à un état étranger).
    let mode_dir = test_dir("autre_trace");
    let archive = build_archive(
        "t-origine",
        false,
        default_audit_params(),
        make_points(2),
        Vec::new(),
    );
    save_archive(&archive_path(&mode_dir, "t-accueil"), &archive).expect("écriture de l'archive");

    assert!(load_archive_impl(&mode_dir, "t-accueil").is_none());
}
