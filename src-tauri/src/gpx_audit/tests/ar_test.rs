//! Tests du détecteur AR.
//!
//! Sous-étapes 1.1 (`compute_headings`), 1.2 (`find_candidates`),
//! 1.3 (`group_candidates`), 1.4 (`find_mirror_pairs`) et 1.5 (`detect_ar`).

use crate::gpx_audit::ar::{
    compute_headings, detect_ar, find_candidates, find_mirror_pairs, group_candidates, Candidate,
};
use crate::gpx_audit::consolidation::consolidate_points;
use crate::gpx_audit::geometry::build_geometry;
use crate::gpx_audit::types::{AuditParams, AuditPoint, Finding, FindingKind, FindingStatus};
use std::fs::File;
use std::io::BufReader;

/// Compare deux listes de caps avec une tolérance flottante (les azimuts
/// 90°/270° résultent d'un `atan2` qui n'est pas exact en virgule flottante).
fn assert_heads_eq(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len(), "nombre de caps inattendu");
    for (a, e) in actual.iter().zip(expected.iter()) {
        assert!(
            (a - e).abs() < 1e-9,
            "cap attendu ~{e}, obtenu {a}"
        );
    }
}

#[test]
fn test_compute_headings_linear() {
    // 3 points alignés vers le nord : px constant, py croissant -> cap 0° (nord).
    let px = [0.0, 0.0, 0.0];
    let py = [0.0, 1.0, 2.0];
    let head = compute_headings(&px, &py);
    assert_heads_eq(&head, &[0.0, 0.0]);
}

#[test]
fn test_compute_headings_east() {
    // 3 points alignés vers l'est : px croissant, py constant -> cap 90° (est).
    let px = [0.0, 1.0, 2.0];
    let py = [0.0, 0.0, 0.0];
    let head = compute_headings(&px, &py);
    assert_heads_eq(&head, &[90.0, 90.0]);
}

#[test]
fn test_compute_headings_eps() {
    // Filet EPS : un micro-segment en tête (report arrière) et un au milieu
    // (report avant) reprennent le cap du voisin.
    // S0 (< EPS) -> report arrière = 0° ; S1 nord = 0° ; S2 est = 90° ;
    // S3 (< EPS) -> report avant = 90°.
    let px = [0.0, 0.0, 0.0, 100.0, 100.0];
    let py = [0.0, 0.001, 100.001, 100.001, 100.002];
    let head = compute_headings(&px, &py);
    assert_heads_eq(&head, &[0.0, 0.0, 90.0, 90.0]);
}

// ─── Sous-étape 1.2 : find_candidates ────────────────────────────────

/// Paramètres AR par défaut (spec §3.1 : p-tol=20, p-pair=50, p-maxpairs=5, p-seg=200).
fn default_params() -> AuditParams {
    AuditParams {
        consol_m: 0.5,
        tol_deg: 20.0,
        pair_m: 50.0,
        maxpairs: 5,
        seg_m: 200.0,
        close_m: 15.0,
        angle_deg: 270,
    }
}

/// Compare une valeur flottante à une valeur attendue (tolérance 1e-9).
fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "valeur attendue ~{expected}, obtenue {actual}"
    );
}

#[test]
fn test_find_candidates_single_uturn() {
    // Demi-tour à 180° au point 5 (aller vers l'est, retour vers l'ouest).
    let px = [0.0, 100.0, 200.0, 300.0, 400.0, 500.0, 400.0, 300.0];
    let py = [0.0; 8];
    let headings = [90.0, 90.0, 90.0, 90.0, 90.0, 270.0, 270.0];
    let cands = find_candidates(&px, &py, &headings, 1, &default_params());
    assert_eq!(cands.len(), 1);
    let c = &cands[0];
    assert_eq!(c.i, 5);
    assert_close(c.diff, 180.0);
    assert_close(c.d1, 100.0);
    assert_close(c.d2, 100.0);
}

#[test]
fn test_find_candidates_branches_too_long() {
    // Demi-tour au point 2, mais branche amont de 500 m (> p-seg = 200 m) : rejet C2.
    let px = [0.0, 500.0, 1000.0, 500.0, 0.0];
    let py = [0.0; 5];
    let headings = [90.0, 90.0, 270.0, 270.0];
    let cands = find_candidates(&px, &py, &headings, 1, &default_params());
    assert_eq!(cands.len(), 0);
}

#[test]
fn test_find_candidates_margin() {
    // Zigzag : chaque point intérieur est un demi-tour. Avec marge 1 sur 10 points,
    // les candidats couvrent [1, 8] ; les extrémités 0 et 9 ne sont jamais candidates.
    let px = [0.0, 100.0, 0.0, 100.0, 0.0, 100.0, 0.0, 100.0, 0.0, 100.0];
    let py = [0.0; 10];
    let headings = [90.0, 270.0, 90.0, 270.0, 90.0, 270.0, 90.0, 270.0, 90.0];
    let cands = find_candidates(&px, &py, &headings, 1, &default_params());
    assert_eq!(cands.len(), 8);
    for c in &cands {
        assert!(c.i >= 1 && c.i <= 8, "index hors marge : {}", c.i);
    }
}

// ─── Sous-étape 1.3 : group_candidates ────────────────────────────────

/// Construit un candidat sommet de test (branches amont/aval fixes à 100 m).
fn cand(i: usize, diff: f64) -> Candidate {
    Candidate {
        i,
        diff,
        d1: 100.0,
        d2: 100.0,
    }
}

#[test]
fn test_group_candidates_r1_chain() {
    // 5 candidats consécutifs (i = 10..14) : R1 fusionne tout en un seul groupe,
    // représentant = candidat médian (i = 12).
    let cands = vec![
        cand(10, 180.0),
        cand(11, 180.0),
        cand(12, 180.0),
        cand(13, 180.0),
        cand(14, 180.0),
    ];
    let headings = vec![0.0; 15];
    let px = vec![0.0; 15];
    let py = vec![0.0; 15];
    let peaks = group_candidates(&cands, &headings, &px, &py, &default_params());
    assert_eq!(peaks.len(), 1);
    assert_eq!(peaks[0].i, 12);
}

#[test]
fn test_group_candidates_r2_fuse() {
    // 2 candidats à i = 5 et i = 9 (écart 4 ≤ FUSE_GAP), caps jumeaux -> fusion R2.
    let cands = vec![cand(5, 150.0), cand(9, 170.0)];
    // head[4] et head[9] identiques (caps jumeaux), tandis que far = 1000 m > pair_m.
    let headings = vec![90.0; 10];
    let px = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1000.0];
    let py = [0.0; 10];
    let peaks = group_candidates(&cands, &headings, &px, &py, &default_params());
    assert_eq!(peaks.len(), 1);
    // Égalité de distance au milieu (7.0) : le diff maximal l'emporte -> i = 9.
    assert_eq!(peaks[0].i, 9);
}

#[test]
fn test_group_candidates_median() {
    // 3 candidats i = 10, 11, 12 / milieu = 11 -> représentant = i = 11,
    // même si son diff est inférieur (le plus proche du milieu prime).
    let cands = vec![cand(10, 180.0), cand(11, 50.0), cand(12, 180.0)];
    let headings = vec![0.0; 13];
    let px = vec![0.0; 13];
    let py = vec![0.0; 13];
    let peaks = group_candidates(&cands, &headings, &px, &py, &default_params());
    assert_eq!(peaks.len(), 1);
    assert_eq!(peaks[0].i, 11);
}

// ─── Sous-étape 1.4 : find_mirror_pairs ───────────────────────────────

#[test]
fn test_find_mirror_pairs_zero_distance() {
    // Aller-retour exact : paires miroirs à distance 0,0 m autour du sommet 5.
    let px = [0.0, 100.0, 200.0, 300.0, 400.0, 500.0, 400.0, 300.0, 200.0, 100.0];
    let py = [0.0; 10];
    let pairs = find_mirror_pairs(5, &px, &py, &default_params(), None);
    assert_eq!(pairs.len(), 3);
    for p in &pairs {
        assert_close(p.d, 0.0);
    }
    assert_eq!((pairs[0].a, pairs[0].b), (3, 7));
    assert_eq!((pairs[1].a, pairs[1].b), (2, 8));
    assert_eq!((pairs[2].a, pairs[2].b), (1, 9));
}

#[test]
fn test_find_mirror_pairs_stops_on_first_reject() {
    // Paires à 0, 0 puis 100 m : la 3e (100 m > p-pair = 50) stoppe la recherche.
    let px = [0.0, 100.0, 200.0, 300.0, 400.0, 500.0, 400.0, 300.0, 200.0, 0.0];
    let py = [0.0; 10];
    let pairs = find_mirror_pairs(5, &px, &py, &default_params(), None);
    assert_eq!(pairs.len(), 2);
    assert_close(pairs[0].d, 0.0);
    assert_close(pairs[1].d, 0.0);
}

#[test]
fn test_find_mirror_pairs_budget_kmax() {
    // 10 paires possibles (tous points confondus), budget p-maxpairs = 5 -> 5 paires.
    let px = vec![0.0; 23];
    let py = vec![0.0; 23];
    let pairs = find_mirror_pairs(11, &px, &py, &default_params(), None);
    assert_eq!(pairs.len(), 5);
}

// ─── Sous-étape 1.5 : detect_ar (assemblage) ──────────────────────────

/// Construit un point d'audit minimal.
fn pt(id: u32, lat: f64, lon: f64) -> AuditPoint {
    AuditPoint {
        id,
        lat,
        lon,
        ele: None,
    }
}

/// Compare une valeur flottante à une valeur attendue avec une tolérance donnée.
fn assert_close_tol(actual: f64, expected: f64, tol: f64) {
    assert!(
        (actual - expected).abs() < tol,
        "valeur attendue ~{expected} (±{tol}), obtenue {actual}"
    );
}

/// Charge le scénario §17.1 (27 points, ids 0..26).
fn load_scenario_17_1() -> Vec<AuditPoint> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../docs/audit/reference/test_files/scenario_17_1_ar_27pts.gpx"
    );
    let file = File::open(path).expect("fichier de scénario introuvable");
    let gpx = gpx::read(BufReader::new(file)).expect("GPX invalide");
    let mut points = Vec::new();
    for track in &gpx.tracks {
        for segment in &track.segments {
            for wp in &segment.points {
                let p = wp.point();
                points.push(pt(points.len() as u32, p.y(), p.x()));
            }
        }
    }
    points
}

/// Consolidation + géométrie + détection AR. Retourne (supprimés, conservés, findings).
fn run_detect(raw: &[AuditPoint], params: &AuditParams) -> (usize, Vec<AuditPoint>, Vec<Finding>) {
    let (kept, removed) = consolidate_points(raw, params.consol_m);
    let geo = build_geometry(&kept);
    let findings = detect_ar(&kept, &geo.px, &geo.py, &geo.ids, params);
    (removed, kept, findings)
}

#[test]
fn test_detect_ar_scenario_17_1() {
    let raw = load_scenario_17_1();
    assert_eq!(raw.len(), 27);
    let (removed, kept, findings) = run_detect(&raw, &default_params());

    assert_eq!(removed, 4);
    assert_eq!(kept.len(), 23);
    assert_eq!(findings.len(), 1);

    let f = &findings[0];
    assert_eq!(f.kind, FindingKind::Ar);
    assert_eq!(f.id, "ar-1");
    assert_eq!(f.label, "Aller-retour : 1");
    assert_eq!(f.peak, 11);
    assert_eq!(f.parts[0].s, 7);
    assert_eq!(f.parts[0].e, 15);

    assert_eq!(f.pairs.len(), 3);
    assert_eq!((f.pairs[0].a, f.pairs[0].b), (9, 13));
    assert_eq!((f.pairs[1].a, f.pairs[1].b), (8, 14));
    assert_eq!((f.pairs[2].a, f.pairs[2].b), (7, 15));
    assert_eq!(f.pairs[0].d, 0.0);
    assert_eq!(f.pairs[1].d, 0.0);
    assert_close_tol(f.pairs[2].d, 40.0, 0.1);

    assert_eq!(f.ctx.up, Some(6));
    assert_eq!(f.ctx.dn, Some(16));

    assert_eq!(f.summary, "sommet pt 12 · 3 paires · écart 0°");
    assert_eq!(f.parts[0].text, "paires miroirs : pts 10↔14 (0.0 m) · pts 9↔15 (0.0 m) · pts 8↔16 (40 m)");
    assert_eq!(f.parts[1].text, "cœur : demi-tour · branches 39/39 m");

    assert_close(f.ecart.unwrap(), 0.0);
    assert_close_tol(f.d1.unwrap(), 39.36, 0.1);
    assert_close_tol(f.d2.unwrap(), 39.36, 0.1);

    assert_eq!(f.status, FindingStatus::Pending);
    assert!(f.correction.is_none());
    assert!(f.undo.is_none());

    assert_eq!(f.pair_idx, vec![9, 13, 8, 14, 7, 15]);
    assert_eq!(f.peak_id, kept[11].id);
    assert_eq!(
        f.zone_ids,
        kept[7..=15].iter().map(|p| p.id).collect::<Vec<_>>()
    );
}

#[test]
fn test_detect_ar_scenario_17_1_p_pair_30() {
    let raw = load_scenario_17_1();
    let mut params = default_params();
    params.pair_m = 30.0;

    let (_removed, _kept, findings) = run_detect(&raw, &params);
    assert_eq!(findings.len(), 1);
    let f = &findings[0];
    assert_eq!(f.peak, 11);
    assert_eq!(f.pairs.len(), 2);
    assert_eq!((f.pairs[0].a, f.pairs[0].b), (9, 13));
    assert_eq!((f.pairs[1].a, f.pairs[1].b), (8, 14));
    assert_eq!(f.parts[0].s, 8);
    assert_eq!(f.parts[0].e, 14);
    assert_eq!(f.ctx.up, Some(7));
    assert_eq!(f.ctx.dn, Some(15));
}

#[test]
fn test_detect_ar_scenario_17_1_p_maxpairs_1() {
    let raw = load_scenario_17_1();
    let mut params = default_params();
    params.maxpairs = 1;

    let (_removed, _kept, findings) = run_detect(&raw, &params);
    assert_eq!(findings.len(), 1);
    let f = &findings[0];
    assert_eq!(f.peak, 11);
    assert_eq!(f.pairs.len(), 1);
    assert_eq!((f.pairs[0].a, f.pairs[0].b), (9, 13));
    assert_eq!(f.parts[0].s, 9);
    assert_eq!(f.parts[0].e, 13);
    assert_eq!(f.ctx.up, Some(8));
    assert_eq!(f.ctx.dn, Some(14));
}

#[test]
fn test_detect_ar_scenario_17_1_p_consol_0() {
    let raw = load_scenario_17_1();
    let mut params = default_params();
    params.consol_m = 0.0;

    let (removed, kept, findings) = run_detect(&raw, &params);
    // σ = 0 : aucune consolidation (27 points analysés, filet EPS opérationnel).
    assert_eq!(removed, 0);
    assert_eq!(kept.len(), 27);
    // Le demi-tour principal reste détecté.
    assert!(findings.len() >= 1);
}

#[test]
fn test_detect_ar_empty() {
    // Trace < 5 points -> aucun finding.
    let points: Vec<AuditPoint> = (0..4)
        .map(|i| pt(i as u32, 45.0, 2.0 + i as f64 * 0.001))
        .collect();
    let geo = build_geometry(&points);
    let findings = detect_ar(&points, &geo.px, &geo.py, &geo.ids, &default_params());
    assert_eq!(findings.len(), 0);
}

#[test]
fn test_detect_ar_immobile() {
    // Trace immobile (tous points identiques) -> aucun finding.
    let points: Vec<AuditPoint> = (0..6).map(|i| pt(i as u32, 45.0, 2.0)).collect();
    let geo = build_geometry(&points);
    let findings = detect_ar(&points, &geo.px, &geo.py, &geo.ids, &default_params());
    assert_eq!(findings.len(), 0);
}
