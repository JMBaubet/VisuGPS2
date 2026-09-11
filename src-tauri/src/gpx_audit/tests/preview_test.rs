//! Tests de l'aperçu de suppression (`preview.rs`) : règles AR (plage directe)
//! et RP (curseurs qui épargnent), chemins publiés et clampage des curseurs.

use crate::gpx_audit::preview::{delete_preview, DeletePreview};
use crate::gpx_audit::types::{
    AuditPoint, Finding, FindingContext, FindingContextIds, FindingKind, FindingPart, FindingStatus,
    PartRole,
};

// ─── Aides ────────────────────────────────────────────────────────────

/// Trace régulière de `n` points vers l'est, pas de 30 m.
fn line_points(n: usize) -> Vec<AuditPoint> {
    (0..n)
        .map(|i| AuditPoint {
            id: (i + 1) as u32,
            lat: 45.0,
            lon: 2.0 + (i as f64) * 0.0004,
            ele: None,
        })
        .collect()
}

/// Boucle circulaire de `n` points, rayon `r` mètres.
fn circle_points(n: usize, r: f64) -> Vec<AuditPoint> {
    let lat0 = 45.0;
    let lon0 = 2.0;
    let m_per_deg_lat = 110540.0;
    let m_per_deg_lon = (lat0 * std::f64::consts::PI / 180.0).cos() * 111320.0;
    (0..n)
        .map(|i| {
            let a = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
            AuditPoint {
                id: (i + 1) as u32,
                lat: lat0 + (r * a.sin()) / m_per_deg_lat,
                lon: lon0 + (r * a.cos()) / m_per_deg_lon,
                ele: None,
            }
        })
        .collect()
}

fn ar_finding(s: usize, e: usize, peak: usize) -> Finding {
    Finding {
        id: "ar-1".to_string(),
        kind: FindingKind::Ar,
        label: "Aller-retour : 1".to_string(),
        summary: String::new(),
        peak,
        peak_id: (peak + 1) as u32,
        pairs: Vec::new(),
        pair_idx: Vec::new(),
        ecart: Some(20.0),
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
        ctx: FindingContext { up: None, dn: None },
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

fn rp_finding(s: usize, e: usize, cs: usize, ce: usize) -> Finding {
    Finding {
        id: "rp-1".to_string(),
        kind: FindingKind::Rp,
        label: "Boucle giratoire : 1".to_string(),
        summary: String::new(),
        peak: cs,
        peak_id: (cs + 1) as u32,
        pairs: Vec::new(),
        pair_idx: Vec::new(),
        ecart: None,
        d1: None,
        d2: None,
        total_angle: Some(360),
        turn_text: None,
        core_ids: Vec::new(),
        zone_ids: Vec::new(),
        ctx_ids: FindingContextIds { up: None, dn: None },
        ctx: FindingContext { up: None, dn: None },
        parts: vec![
            FindingPart {
                s,
                e,
                role: PartRole::Warn,
                text: String::new(),
            },
            FindingPart {
                s: cs,
                e: ce,
                role: PartRole::Warn,
                text: String::new(),
            },
        ],
        status: FindingStatus::Pending,
        correction: None,
        undo: None,
    }
}

/// Nombre de points marqués « conservés » dans l'aperçu.
fn kept_count(p: &DeletePreview) -> usize {
    p.points.iter().filter(|s| s.kept).count()
}

// ─── Aperçu AR ────────────────────────────────────────────────────────

#[test]
fn test_ar_preview_open_state_keeps_nothing_inside() {
    // Sliders aux extrémités : toute l'emprise est supprimée, le chemin bleu
    // relie les deux points de contexte.
    let points = line_points(30);
    let finding = ar_finding(10, 16, 13);

    let p = delete_preview(&points, &finding, 10, 16, 15.0).expect("aperçu AR");
    assert_eq!(p.points.len(), 7);
    assert_eq!(kept_count(&p), 0, "aucun point conservé dans la plage");
    assert!(p.red.is_empty(), "l'aperçu AR n'a pas de trait rouge");
    assert!(p.labels.is_empty(), "les étiquettes AR sont gérées par la carte");

    // Contexte amont (9) puis contexte aval (17).
    assert_eq!(p.join.len(), 2);
    assert!((p.join[0].lon - points[9].lon).abs() < 1e-12);
    assert!((p.join[1].lon - points[17].lon).abs() < 1e-12);

    assert_eq!(p.count_start, "pt 11");
    assert_eq!(p.count_end, "pt 17");
}

#[test]
fn test_ar_preview_keeps_points_outside_range() {
    let points = line_points(30);
    let finding = ar_finding(10, 16, 13);

    let p = delete_preview(&points, &finding, 12, 14, 15.0).expect("aperçu AR");
    assert_eq!(kept_count(&p), 4, "10-11 et 15-16 sont conservés");
    let kept: Vec<usize> = p
        .points
        .iter()
        .filter(|s| s.kept)
        .map(|s| s.index)
        .collect();
    assert_eq!(kept, vec![10, 11, 15, 16]);
    // Aucune ancre de routage en AR.
    assert!(p.points.iter().all(|s| !s.is_anchor));
}

#[test]
fn test_ar_preview_join_path_runs_from_context_to_context() {
    let points = line_points(30);
    let finding = ar_finding(10, 16, 13);

    // Plage [12..=13] : conservés 10, 11 puis 14, 15, 16.
    let p = delete_preview(&points, &finding, 12, 13, 15.0).expect("aperçu AR");
    // Amont : contexte (9) → dernier conservé amont (11) = 9,10,11.
    // Aval : premier conservé aval (14) → contexte (17) = 14..17.
    assert_eq!(p.join.len(), 3 + 4);
    assert!((p.join[0].lon - points[9].lon).abs() < 1e-12);
    assert!((p.join[2].lon - points[11].lon).abs() < 1e-12);
    assert!((p.join[3].lon - points[14].lon).abs() < 1e-12);
    assert!((p.join[6].lon - points[17].lon).abs() < 1e-12);
}

#[test]
fn test_ar_preview_clamps_single_point_and_reversed_range() {
    let points = line_points(30);
    let finding = ar_finding(10, 16, 13);

    // Plage inversée : ramenée à un point isolé (règle `start > end → start = end`).
    let p = delete_preview(&points, &finding, 14, 12, 15.0).expect("aperçu AR");
    assert_eq!(p.start, 12);
    assert_eq!(p.end, 12);
    assert_eq!(kept_count(&p), 6);

    // Bornes hors emprise : ramenées dans [s0..e0].
    let p2 = delete_preview(&points, &finding, 3, 40, 15.0).expect("aperçu AR clampé");
    assert_eq!(p2.start, 10);
    assert_eq!(p2.end, 16);
}

#[test]
fn test_ar_preview_cursors_fall_back_on_trace_edges() {
    let points = line_points(30);
    // Emprise collée au début de la trace : pas de contexte amont.
    let finding = ar_finding(0, 4, 2);

    let p = delete_preview(&points, &finding, 0, 4, 15.0).expect("aperçu AR en tête");
    assert_eq!(p.cursors[0].title, "bord de trace");
    assert_eq!(p.cursors[0].index, 0);
    // Le curseur aval est posé sur le conservé 5 (= end + 1 ≤ m-1).
    assert_eq!(p.cursors[1].title, "conservé");
    assert_eq!(p.cursors[1].index, 5);
    // Côté amont, le contexte n'existe pas (indice −1) : le chemin se réduit au
    // seul point aval, donc à aucune polyligne (< 2 points).
    assert!(p.join.is_empty(), "aucune polyligne pour un point isolé");
}

// ─── Aperçu RP ────────────────────────────────────────────────────────

#[test]
fn test_rp_preview_open_state_keeps_only_the_bounds() {
    let points = circle_points(60, 40.0);
    let finding = rp_finding(5, 50, 10, 45);

    let p = delete_preview(&points, &finding, 5, 50, 15.0).expect("aperçu RP");
    // À l'ouverture, seules les deux bornes d'emprise sont conservées.
    let kept: Vec<usize> = p
        .points
        .iter()
        .filter(|s| s.kept)
        .map(|s| s.index)
        .collect();
    assert_eq!(kept, vec![5, 50]);
    // Plage supprimée = [début+1 .. fin-1] = 6..49.
    assert_eq!(p.red.len(), 44);
    assert_eq!(p.red[0].lon, points[6].lon);
    // Chemin bleu : bornes + corde directe entre les deux curseurs.
    assert_eq!(p.join.len(), 2);
    // Étiquettes des conservés, classe `keep`.
    assert_eq!(p.labels.len(), 2);
    assert!(p.labels.iter().all(|l| l.cls == "keep"));
    assert_eq!(p.count_start, "Points conservés : [6 – 6]");
    assert_eq!(p.count_end, "Points conservés : [51 – 51]");
}

#[test]
fn test_rp_preview_sparing_points_shrinks_the_red_range() {
    let points = circle_points(60, 40.0);
    let finding = rp_finding(5, 50, 10, 45);

    // Curseur Fin ramené : les points 30..50 deviennent conservés.
    let p = delete_preview(&points, &finding, 5, 30, 15.0).expect("aperçu RP");
    let kept: Vec<usize> = p
        .points
        .iter()
        .filter(|s| s.kept)
        .map(|s| s.index)
        .collect();
    assert_eq!(kept, vec![5, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50]);
    // Le rouge s'est contracté à [6..29].
    assert_eq!(p.red.len(), 24);
    assert_eq!(p.labels.len(), kept.len());
}

#[test]
fn test_rp_preview_marks_anchors_and_destylizes_them() {
    let points = circle_points(80, 40.0);
    let finding = rp_finding(0, 79, 5, 70);

    let p = delete_preview(&points, &finding, 0, 79, 15.0).expect("aperçu RP");
    // Les deux ancres de routage sont signalées (destylisées côté front).
    let anchors: Vec<usize> = p
        .points
        .iter()
        .filter(|s| s.is_anchor)
        .map(|s| s.index)
        .collect();
    assert_eq!(anchors.len(), 2, "ancres : {anchors:?}");
    assert!(anchors[0] < anchors[1]);
}

#[test]
fn test_rp_preview_clamp_never_crosses_cursors() {
    let points = circle_points(60, 40.0);
    let finding = rp_finding(5, 50, 10, 45);

    // Curseur Fin < début + 1 : reculé pour laisser au moins un point supprimé.
    let p = delete_preview(&points, &finding, 20, 20, 15.0).expect("aperçu RP clampé");
    assert!(p.end >= p.start + 1, "fin {} début {}", p.end, p.start);
    // La plage supprimée contient au moins un point.
    assert!(!p.red.is_empty() || p.red.len() < 2);

    // Curseurs collés aux bornes puis échangés : jamais croisés.
    let p2 = delete_preview(&points, &finding, 50, 5, 15.0).expect("aperçu RP inversé");
    assert!(p2.start <= p2.end);
    assert!(p2.start >= 5 && p2.end <= 50);
}

#[test]
fn test_rp_preview_empty_red_range_when_nothing_to_delete() {
    let points = circle_points(60, 40.0);
    let finding = rp_finding(5, 50, 10, 45);

    // fin = début + 1 : plus rien à supprimer entre les curseurs.
    let p = delete_preview(&points, &finding, 20, 21, 15.0).expect("aperçu RP vide");
    assert!(p.red.is_empty(), "aucun trait rouge pour une plage vide");
    assert!(p.join.len() >= 2);
}

// ─── Cas d'erreur ─────────────────────────────────────────────────────

#[test]
fn test_preview_rejects_empty_trace() {
    let finding = ar_finding(0, 4, 2);
    let err = delete_preview(&[], &finding, 0, 1, 15.0).expect_err("trace vide");
    assert!(err.contains("vide"), "message inattendu : {err}");
}

#[test]
fn test_preview_rejects_finding_without_zone() {
    let points = line_points(30);
    let mut finding = ar_finding(0, 4, 2);
    finding.parts.clear();
    let err = delete_preview(&points, &finding, 0, 1, 15.0).expect_err("sans emprise");
    assert!(err.contains("emprise"), "message inattendu : {err}");
}

#[test]
fn test_preview_rejects_zone_out_of_trace() {
    let points = line_points(10);
    let finding = ar_finding(0, 20, 2);
    let err = delete_preview(&points, &finding, 0, 1, 15.0).expect_err("emprise hors trace");
    assert!(err.contains("hors de la trace"), "message inattendu : {err}");
}

#[test]
fn test_rp_preview_rejects_missing_core() {
    let points = circle_points(60, 40.0);
    let mut finding = rp_finding(5, 50, 10, 45);
    finding.parts.truncate(1);
    let err = delete_preview(&points, &finding, 5, 50, 15.0).expect_err("RP sans cœur");
    assert!(err.contains("cœur"), "message inattendu : {err}");
}
