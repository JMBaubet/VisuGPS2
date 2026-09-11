//! Tests des éléments de rendu (`overlay.rs`) : ancres de routage RP et
//! placement des étiquettes (éventail AR, règle radiale RP, points d'origine).

use crate::gpx_audit::overlay::{
    map_overlay, radial_offsets, rp_anchors, LABEL_MAX, LABEL_DUP_M,
};
use crate::gpx_audit::types::{
    AuditPoint, CorrectionType, Finding, FindingContext, FindingContextIds, FindingKind,
    FindingPart, FindingStatus, PartRole,
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

/// Boucle circulaire de `n` points autour de (lat0, lon0), rayon `r` mètres.
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

/// Finding AR minimal couvrant `[s..=e]` avec sommet `peak`.
fn ar_finding(s: usize, e: usize, peak: usize, status: FindingStatus) -> Finding {
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
            up: Some(peak as u32),
            dn: Some((peak + 2) as u32),
        },
        // Le détecteur place le contexte **hors** de l'emprise (s−1 / e+1).
        ctx: FindingContext {
            up: s.checked_sub(1),
            dn: Some(e + 1),
        },
        parts: vec![FindingPart {
            s,
            e,
            role: PartRole::Warn,
            text: String::new(),
        }],
        status,
        correction: None,
        undo: None,
    }
}

/// Finding RP minimal : emprise `[s..=e]`, cœur `[cs..=ce]`, jonction = `cs`.
fn rp_finding(s: usize, e: usize, cs: usize, ce: usize, status: FindingStatus) -> Finding {
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
        ctx_ids: FindingContextIds {
            up: None,
            dn: None,
        },
        ctx: FindingContext {
            up: None,
            dn: None,
        },
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
        status,
        correction: None,
        undo: None,
    }
}

// ─── radial_offsets ───────────────────────────────────────────────────

#[test]
fn test_radial_offsets_alternate_rings() {
    // Trois points alignés espacés de 1 m (< LABEL_DUP_M), centre loin au nord.
    // Le 2ᵉ voit le 1ᵉʳ comme jumeau métrique : couronne +24 (84 → 108).
    let pts = [(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)];
    let offs = radial_offsets(&pts, 0.0, 100.0);

    assert_eq!(offs.len(), 3);
    assert!((offs[0].0 - 0.0).abs() < 1e-9, "dx0 = {}", offs[0].0);
    assert!((offs[0].1 - 60.0).abs() < 1e-9, "dy0 = {}", offs[0].1);
    // Normal extérieure (0, −1) : l'écran descend de +108.
    assert!((offs[1].0 - 0.0).abs() < 1e-9, "dx1 = {}", offs[1].0);
    assert!((offs[1].1 - 108.0).abs() < 1e-9, "dy1 = {}", offs[1].1);
}

#[test]
fn test_radial_offsets_fallback_when_tangent_undefined() {
    // Points confondus : la tangente est indéterminée, repli radial.
    let pts = [(10.0, 0.0)];
    let offs = radial_offsets(&pts, 0.0, 0.0);
    assert!((offs[0].0 - 60.0).abs() < 1e-9, "dx = {}", offs[0].0);
    assert!((offs[0].1 - 0.0).abs() < 1e-9, "dy = {}", offs[0].1);
}

#[test]
fn test_labal_dup_threshold_is_eight_metres() {
    assert!((LABEL_DUP_M - 8.0).abs() < 1e-9);
}

// ─── Ancres RP ────────────────────────────────────────────────────────

#[test]
fn test_rp_anchors_are_clamped_inside_zone() {
    let pts = circle_points(80, 40.0);
    let geo = crate::gpx_audit::geometry::build_geometry(&pts);
    let proj = crate::gpx_audit::geometry::Projector::new(&pts[0]);

    let (s, e, cs, ce) = (0usize, 79usize, 5usize, 70usize);
    let an = rp_anchors(&geo.px, &geo.py, &proj, cs, ce, s, e, 15.0);

    // Invariants de `rpAnchors` : l'amont précède la jonction et reste dans
    // l'emprise ; l'aval suit la fin du cœur et reste dans l'emprise.
    assert!(an.up >= s, "up {} < s {}", an.up, s);
    assert!(an.up <= cs.saturating_sub(1), "up {} > cs-1", an.up);
    assert!(an.dn >= ce + 1, "dn {} < ce+1", an.dn);
    assert!(an.dn <= e, "dn {} > e", an.dn);

    // Boucle franche : le cercle ajusté existe et son rayon est plausible.
    assert!(an.center.is_some(), "centre attendu sur une boucle franche");
    if let Some(r) = an.radius_m {
        assert!((20.0..80.0).contains(&r), "rayon inattendu : {r}");
    }
}

#[test]
fn test_rp_anchors_are_clamped_against_out_of_range_extension() {
    // Emprise volontairement plus étroite que l'extension radiale naturelle :
    // le clampage doit ramener les ancres dans [s..=e].
    let pts = circle_points(80, 40.0);
    let geo = crate::gpx_audit::geometry::build_geometry(&pts);
    let proj = crate::gpx_audit::geometry::Projector::new(&pts[0]);

    let (s, e, cs, ce) = (40usize, 60usize, 45usize, 55usize);
    let an = rp_anchors(&geo.px, &geo.py, &proj, cs, ce, s, e, 15.0);
    assert!(an.up >= s && an.up <= cs.saturating_sub(1));
    assert!(an.dn >= ce + 1 && an.dn <= e);
}

// ─── Étiquettes d'une anomalie non corrigée ───────────────────────────

#[test]
fn test_map_overlay_ar_fan_layout() {
    let points = line_points(20);
    let finding = ar_finding(5, 9, 7, FindingStatus::Pending);

    let ov = map_overlay(&points, &finding, 15.0).expect("rendu AR");
    assert!(ov.anchors.is_none(), "pas d'ancre de routage pour un AR");

    // L'éventail émet exactement une étiquette par point de l'emprise [5..=9]
    // (sommet compris), plus les deux points de contexte sains (s−1 et e+1).
    assert_eq!(ov.labels.len(), (9 - 5 + 1) + 2);

    // Ordre du JS : éventail (amont → aval, sommet exclu), puis sommet, puis
    // contexte amont et aval.
    assert_eq!(ov.labels[0].index, 5);
    assert_eq!(ov.labels[3].index, 9);

    let peak = &ov.labels[4];
    assert_eq!(peak.index, 7, "le sommet suit l'éventail");
    assert_eq!(peak.cls, "pk");
    assert_eq!(peak.no, 8, "numéro affiché = indice + 1");
    assert!((peak.dx - 0.0).abs() < 1e-9, "dx(sommet) = {}", peak.dx);
    assert!((peak.dy + 84.0).abs() < 1e-9, "dy(sommet) = {}", peak.dy);

    // Rang 1 en amont : (−28, −34) ; rang 2 : (−48, −58).
    let j6 = ov.labels.iter().find(|l| l.index == 6).expect("pt 6");
    assert!((j6.dx + 28.0).abs() < 1e-9, "dx(pt6) = {}", j6.dx);
    assert!((j6.dy + 34.0).abs() < 1e-9, "dy(pt6) = {}", j6.dy);

    let j5 = ov.labels.iter().find(|l| l.index == 5).expect("pt 5");
    assert!((j5.dx + 48.0).abs() < 1e-9, "dx(pt5) = {}", j5.dx);
    assert!((j5.dy + 58.0).abs() < 1e-9, "dy(pt5) = {}", j5.dy);

    // Contexte sain, en prolongement de l'éventail de part et d'autre.
    let cu = &ov.labels[5];
    assert_eq!(cu.index, 4);
    assert_eq!(cu.cls, "ctx");
    assert!((cu.dx + 88.0).abs() < 1e-9, "dx(ctx amont) = {}", cu.dx);
    assert!((cu.dy + 34.0).abs() < 1e-9, "dy(ctx amont) = {}", cu.dy);

    let cd = &ov.labels[6];
    assert_eq!(cd.index, 10);
    assert_eq!(cd.cls, "ctx");
    assert!((cd.dx - 88.0).abs() < 1e-9, "dx(ctx aval) = {}", cd.dx);
    assert!((cd.dy - 34.0).abs() < 1e-9, "dy(ctx aval) = {}", cd.dy);
}

#[test]
fn test_map_overlay_ar_skips_missing_context() {
    // Emprise collée au début de la trace : pas de contexte amont.
    let points = line_points(20);
    let mut finding = ar_finding(0, 4, 2, FindingStatus::Pending);
    finding.ctx.up = None;

    let ov = map_overlay(&points, &finding, 15.0).expect("rendu AR");
    assert_eq!(ov.labels.len(), 5 + 1, "seul le contexte aval subsiste");
    assert_eq!(ov.labels.iter().filter(|l| l.cls == "ctx").count(), 1);
}

#[test]
fn test_map_overlay_ar_faux_positif_has_no_context() {
    let points = line_points(20);
    let finding = ar_finding(5, 9, 7, FindingStatus::Fp);

    let ov = map_overlay(&points, &finding, 15.0).expect("rendu AR fp");
    assert!(ov.labels.iter().all(|l| l.cls != "ctx"), "aucun contexte en FP");
    assert!(ov.labels.iter().all(|l| l.cls == "fpl"));
}

#[test]
fn test_map_overlay_rp_labels_include_landmarks() {
    let points = circle_points(80, 40.0);
    let (s, e, cs, ce) = (0usize, 79usize, 5usize, 70usize);
    let finding = rp_finding(s, e, cs, ce, FindingStatus::Pending);

    let ov = map_overlay(&points, &finding, 15.0).expect("rendu RP");
    let anchors = ov.anchors.as_ref().expect("ancres RP");

    // Les jalons sont systématiquement étiquetés : bornes, jonction, ancres.
    let idx: Vec<usize> = ov.labels.iter().map(|l| l.index).collect();
    for k in [s, e, cs, anchors.up, anchors.dn] {
        assert!(idx.contains(&k), "jalon {k} absent des étiquettes");
    }
    // Indices triés et uniques.
    let mut sorted = idx.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted, idx, "les indices doivent être triés et sans doublon");
    // La classe reste neutre hors faux positif.
    assert!(ov.labels.iter().all(|l| l.cls.is_empty()));
}

#[test]
fn test_map_overlay_rp_stride_caps_label_count() {
    // Emprise de 400 points : le stride doit plafonner le nombre d'étiquettes.
    let points = circle_points(400, 60.0);
    let (s, e, cs, ce) = (0usize, 399usize, 10usize, 380usize);
    let finding = rp_finding(s, e, cs, ce, FindingStatus::Pending);

    let ov = map_overlay(&points, &finding, 15.0).expect("rendu RP long");
    let attendu = 400usize.div_ceil(LABEL_MAX);
    // Stride 4 → 100 points, plus au plus 5 jalons.
    assert!(
        ov.labels.len() <= 100 + 5,
        "{} étiquettes pour un stride de {attendu}",
        ov.labels.len()
    );
}

#[test]
fn test_map_overlay_rp_faux_positif_uses_fpl_class() {
    let points = circle_points(80, 40.0);
    let finding = rp_finding(0, 79, 5, 70, FindingStatus::Fp);
    let ov = map_overlay(&points, &finding, 15.0).expect("rendu RP fp");
    assert!(!ov.labels.is_empty());
    assert!(ov.labels.iter().all(|l| l.cls == "fpl"));
}

// ─── Étiquettes d'une anomalie corrigée ───────────────────────────────

#[test]
fn test_map_overlay_corrected_ar_labels_use_origin_points() {
    let points = line_points(20);
    let mut finding = ar_finding(5, 9, 7, FindingStatus::Corrected);
    finding.correction = Some(CorrectionType::Delete);
    finding.undo = Some(crate::gpx_audit::types::UndoRecord::Delete(
        crate::gpx_audit::types::UndoDelete {
            orig_pts: line_points(3),
            anchor_left_id: Some(5),
            anchor_right_id: Some(9),
            first_no: 6,
            absorbed_fp: Vec::new(),
        },
    ));

    let ov = map_overlay(&points, &finding, 15.0).expect("rendu corrigé");
    assert!(ov.anchors.is_none());
    assert_eq!(ov.labels.len(), 3);
    assert!(ov.labels.iter().all(|l| l.cls == "del"));
    // Numérotation par rang depuis `firstNo` (le rang 0 porte firstNo).
    assert_eq!(ov.labels[0].no, 6);
    assert_eq!(ov.labels[1].no, 7);
    assert_eq!(ov.labels[2].no, 8);
    // Offsets alternés : (24, −50), (−48, −72), (72, −50).
    assert!((ov.labels[0].dx - 24.0).abs() < 1e-9, "dx0 = {}", ov.labels[0].dx);
    assert!((ov.labels[0].dy + 50.0).abs() < 1e-9, "dy0 = {}", ov.labels[0].dy);
    assert!((ov.labels[1].dx + 48.0).abs() < 1e-9, "dx1 = {}", ov.labels[1].dx);
    assert!((ov.labels[1].dy + 72.0).abs() < 1e-9, "dy1 = {}", ov.labels[1].dy);
}

#[test]
fn test_map_overlay_corrected_rp_keeps_junction_label() {
    let points = circle_points(80, 40.0);
    let orig = circle_points(80, 40.0);

    let mut finding = rp_finding(0, 79, 5, 70, FindingStatus::Corrected);
    finding.correction = Some(CorrectionType::Delete);
    // L'ex-jonction est repérée par `peak_id` (id stable du point 5 → 6).
    finding.peak_id = orig[5].id;
    finding.undo = Some(crate::gpx_audit::types::UndoRecord::Delete(
        crate::gpx_audit::types::UndoDelete {
            orig_pts: orig.clone(),
            anchor_left_id: None,
            anchor_right_id: None,
            first_no: 1,
            absorbed_fp: Vec::new(),
        },
    ));

    let ov = map_overlay(&points, &finding, 15.0).expect("rendu corrigé RP");
    assert!(ov.labels.iter().all(|l| l.cls == "del"));
    // L'ex-jonction (rang 5) est systématiquement étiquetée.
    assert!(
        ov.labels.iter().any(|l| l.index == 5),
        "l'ex-jonction doit être étiquetée"
    );
    assert!(ov.labels.iter().any(|l| l.index == 0));
    assert!(ov.labels.iter().any(|l| l.index == orig.len() - 1));
}

// ─── Cas d'erreur ─────────────────────────────────────────────────────

#[test]
fn test_map_overlay_rejects_empty_trace() {
    let finding = ar_finding(0, 1, 0, FindingStatus::Pending);
    let err = map_overlay(&[], &finding, 15.0).expect_err("trace vide refusée");
    assert!(err.contains("vide"), "message inattendu : {err}");
}

#[test]
fn test_map_overlay_rejects_finding_without_part() {
    let points = line_points(20);
    let mut finding = ar_finding(5, 9, 7, FindingStatus::Pending);
    finding.parts.clear();
    let err = map_overlay(&points, &finding, 15.0).expect_err("finding sans emprise");
    assert!(err.contains("emprise"), "message inattendu : {err}");
}

#[test]
fn test_map_overlay_rejects_rp_without_core() {
    let points = circle_points(80, 40.0);
    let mut finding = rp_finding(0, 79, 5, 70, FindingStatus::Pending);
    finding.parts.truncate(1);
    let err = map_overlay(&points, &finding, 15.0).expect_err("RP sans cœur");
    assert!(err.contains("cœur"), "message inattendu : {err}");
}

// ─── Rendu de l'ensemble des anomalies ────────────────────────────────

#[test]
fn test_map_overlays_returns_one_entry_per_finding() {
    let points = circle_points(80, 40.0);
    let findings = vec![
        ar_finding(2, 8, 5, FindingStatus::Pending),
        rp_finding(0, 79, 5, 70, FindingStatus::Pending),
        ar_finding(12, 18, 15, FindingStatus::Fp),
    ];

    let ovs = crate::gpx_audit::overlay::map_overlays(&points, &findings, 15.0)
        .expect("rendu de l'ensemble");
    assert_eq!(ovs.len(), 3);
    assert_eq!(ovs[0].id, "ar-1");
    assert_eq!(ovs[1].id, "rp-1");
    assert_eq!(ovs[2].id, "ar-1");

    // Seule la boucle porte des ancres de routage.
    assert!(ovs[0].anchors.is_none());
    assert!(ovs[1].anchors.is_some());
    assert!(ovs[2].anchors.is_none());
    // Chaque anomalie est étiquetée.
    assert!(ovs.iter().all(|o| !o.labels.is_empty()));
}

#[test]
fn test_map_overlays_skips_malformed_finding() {
    let points = line_points(20);
    let mut broken = ar_finding(5, 9, 7, FindingStatus::Pending);
    broken.id = "ar-ko".to_string();
    broken.parts.clear();

    let findings = vec![broken, ar_finding(1, 4, 2, FindingStatus::Pending)];
    let ovs = crate::gpx_audit::overlay::map_overlays(&points, &findings, 15.0)
        .expect("le rendu doit survivre à une anomalie mal formée");
    assert_eq!(ovs.len(), 1, "seule l'anomalie rendable est publiée");
    assert_eq!(ovs[0].id, "ar-1");
}

#[test]
fn test_map_overlays_rejects_empty_trace() {
    let err = crate::gpx_audit::overlay::map_overlays(&[], &[], 15.0)
        .expect_err("trace vide refusée");
    assert!(err.contains("vide"), "message inattendu : {err}");
}
