//! Tests du moteur de correction — sous-étapes 3.1 à 3.5.
//!
//! Couvre le réalignement et l'imbrication (`sync_indexes`, `after_trace_edit`,
//! `rebuild_texts` via §3, `nesting_guard`, `absorb_fp_findings` — CORRECTIONS
//! §3, §4, §9), la suppression de plage (`apply_delete` — §5), le routage
//! (`apply_route` — §6), le faux positif / annulation (`mark_fp`, `unmark_fp`,
//! `undo_correction` — §7, §8), la migration (D1, D2c) et l'export GPX (IHM §20).

use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};

use crate::gpx_audit::consolidation::consolidate_points;
use crate::gpx_audit::geometry::build_geometry;
use crate::gpx_audit::rp::detect_rp;
use crate::gpx_audit::corrections::{
    absorb_fp_findings, after_trace_edit, apply_delete, apply_route, mark_fp, nesting_guard,
    sync_indexes, undo_correction, unmark_fp,
};
use crate::gpx_audit::export::{rewrite_gpx, FindingsSummary};
use crate::gpx_audit::migration::{cleanup_obsolete_cleaning_files, is_obsolete_registry};
use crate::gpx_audit::rp::turn_text;
use crate::gpx_audit::types::{
    AuditParams, AuditPoint, AuditState, CorrectionType, Finding, FindingContext, FindingContextIds, FindingKind,
    FindingPair, FindingPart, FindingStatus, LatLon, PartRole, UndoDelete, UndoRecord, UndoRoute,
};

// ─── Fabriques de test ────────────────────────────────────────────────

/// Trace de test : `n` points d'id `1..=n`, espacés d'environ 111 m vers le nord.
fn make_points(n: usize) -> Vec<AuditPoint> {
    (1..=n)
        .map(|i| AuditPoint {
            id: i as u32,
            lat: 45.0 + (i as f64) * 0.001,
            lon: 3.0,
            ele: Some(100.0),
        })
        .collect()
}

/// Retire de `points` la plage d'index `[ds..=de]` (suppression brute, comme
/// le ferait `apply_delete` — §5.3).
fn delete_range(points: &[AuditPoint], ds: usize, de: usize) -> Vec<AuditPoint> {
    points
        .iter()
        .enumerate()
        .filter(|(i, _)| *i < ds || *i > de)
        .map(|(_, p)| p.clone())
        .collect()
}

/// Finding AR minimal, calqué sur la publication du détecteur (`ar.rs`) :
/// emprise `[s..=e]`, sommet `peak`, paires miroirs `(a, b, d)`.
/// Les textes sont laissés vides : `rebuild_texts` les régénère (§2.4).
fn ar_finding(
    id: &str,
    pts: &[AuditPoint],
    s: usize,
    e: usize,
    peak: usize,
    pairs_at: &[(usize, usize, f64)],
) -> Finding {
    let ids: Vec<u32> = pts.iter().map(|p| p.id).collect();
    let pairs: Vec<FindingPair> = pairs_at
        .iter()
        .map(|&(a, b, d)| FindingPair {
            aid: ids[a],
            bid: ids[b],
            a,
            b,
            d,
        })
        .collect();
    let pair_idx = pairs.iter().flat_map(|p| [p.a, p.b]).collect();
    let (d1, d2) = (12.0_f64, 9.0_f64);

    Finding {
        id: id.to_string(),
        kind: FindingKind::Ar,
        label: format!("Aller-retour : {}", id),
        summary: String::new(),
        peak,
        peak_id: ids[peak],
        pairs,
        pair_idx,
        ecart: Some(40.0),
        d1: Some(d1),
        d2: Some(d2),
        total_angle: None,
        turn_text: None,
        core_ids: Vec::new(),
        zone_ids: ids[s..=e].to_vec(),
        ctx_ids: FindingContextIds {
            up: if s >= 1 { Some(ids[s - 1]) } else { None },
            dn: if e + 1 < ids.len() {
                Some(ids[e + 1])
            } else {
                None
            },
        },
        ctx: FindingContext {
            up: if s >= 1 { Some(s - 1) } else { None },
            dn: if e + 1 < ids.len() {
                Some(e + 1)
            } else {
                None
            },
        },
        parts: vec![
            FindingPart {
                s,
                e,
                role: PartRole::Warn,
                text: String::new(),
            },
            FindingPart {
                s: peak - 1,
                e: peak + 1,
                role: PartRole::Info,
                // Texte de cœur : invariant, il porte `d1`/`d2` et aucun numéro.
                text: format!("cœur : demi-tour · branches {:.0}/{:.0} m", d1, d2),
            },
        ],
        status: FindingStatus::Pending,
        correction: None,
        undo: None,
    }
}

/// Finding RP minimal, calqué sur la publication du détecteur (`rp.rs`) :
/// emprise `[s..=e]`, cœur `[cs..=ce]`, jonction en `cs`.
fn rp_finding(
    id: &str,
    pts: &[AuditPoint],
    s: usize,
    e: usize,
    cs: usize,
    ce: usize,
    pairs_at: &[(usize, usize, f64)],
    total_angle: f64,
) -> Finding {
    let ids: Vec<u32> = pts.iter().map(|p| p.id).collect();
    let pairs: Vec<FindingPair> = pairs_at
        .iter()
        .map(|&(a, b, d)| FindingPair {
            aid: ids[a],
            bid: ids[b],
            a,
            b,
            d,
        })
        .collect();
    let pair_idx = pairs.iter().flat_map(|p| [p.a, p.b]).collect();
    let total = total_angle.round() as i32;
    let tt = turn_text(total_angle);

    Finding {
        id: id.to_string(),
        kind: FindingKind::Rp,
        label: format!("Tour de rond-point : {}", id),
        summary: String::new(),
        peak: cs,
        peak_id: ids[cs],
        pairs,
        pair_idx,
        ecart: None,
        d1: None,
        d2: None,
        total_angle: Some(total),
        turn_text: Some(tt),
        core_ids: ids[cs..=ce].to_vec(),
        zone_ids: ids[s..=e].to_vec(),
        ctx_ids: FindingContextIds {
            up: if s >= 1 { Some(ids[s - 1]) } else { None },
            dn: if e + 1 < ids.len() {
                Some(ids[e + 1])
            } else {
                None
            },
        },
        ctx: FindingContext {
            up: if s >= 1 { Some(s - 1) } else { None },
            dn: if e + 1 < ids.len() {
                Some(e + 1)
            } else {
                None
            },
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
        status: FindingStatus::Pending,
        correction: None,
        undo: None,
    }
}

/// Noms des ids d'une liste de findings (pour les comparaisons d'ensembles).
fn ids_of(findings: &[Finding]) -> Vec<String> {
    findings.iter().map(|f| f.id.clone()).collect()
}

/// Ids d'une liste de points (pour les comparaisons de trace).
fn pt_ids(points: &[AuditPoint]) -> Vec<u32> {
    points.iter().map(|p| p.id).collect()
}

/// Extrait l'instantané d'annulation d'un finding, en vérifiant son type.
fn undo_delete_of(f: &Finding) -> &UndoDelete {
    match f.undo.as_ref() {
        Some(UndoRecord::Delete(u)) => u,
        other => panic!("undo attendu de type delete, obtenu {:?}", other),
    }
}

// ─── Sous-étape 3.1 — sync_indexes ────────────────────────────────────

/// D1 — suppression **amont** : les index du finding descendent exactement du
/// nombre de points supprimés, les textes portent les nouveaux numéros et les
/// mesures `d` restent invariantes (CORRECTIONS §3.2).
#[test]
fn test_sync_indexes_after_delete_upstream() {
    let pts = make_points(12);
    let mut findings = vec![ar_finding("ar-1", &pts, 5, 8, 6, &[(5, 7, 40.0)])];

    // Suppression des points d'id 1 et 2 (amont de l'emprise 5..8).
    let working = delete_range(&pts, 0, 1);
    sync_indexes(&working, &mut findings);
    let f = findings.remove(0);

    assert_eq!(f.parts[0].s, 3, "début d'emprise translaté");
    assert_eq!(f.parts[0].e, 6, "fin d'emprise translatée");
    assert_eq!(f.peak, 4, "sommet translaté");
    // Cœur AR = voisinage du sommet.
    assert_eq!(f.parts[1].s, 3);
    assert_eq!(f.parts[1].e, 5);
    // Contexte translaté (ids 5 et 10).
    assert_eq!(f.ctx.up, Some(2));
    assert_eq!(f.ctx.dn, Some(7));
    // Paire translatée, mesure invariante.
    assert_eq!(f.pairs.len(), 1);
    assert_eq!((f.pairs[0].a, f.pairs[0].b), (3, 5));
    assert_eq!(f.pairs[0].d, 40.0, "la distance mesurée ne bouge pas");
    assert_eq!(f.pair_idx, vec![3, 5]);
    // Textes régénérés avec les nouveaux numéros.
    assert_eq!(f.summary, "sommet pt 5 · 1 paire · écart 40°");
    assert_eq!(f.parts[0].text, "paires miroirs : pts 4↔6 (40 m)");
}

/// D2 — suppression **aval** : les index du finding restent inchangés
/// (l'édition est postérieure à son emprise).
#[test]
fn test_sync_indexes_after_delete_downstream() {
    let pts = make_points(12);
    let mut findings = vec![ar_finding("ar-1", &pts, 5, 8, 6, &[(5, 7, 40.0)])];

    // Suppression des deux derniers points, en aval de l'emprise.
    let working = delete_range(&pts, 10, 11);
    sync_indexes(&working, &mut findings);

    let f = &findings[0];
    assert_eq!((f.parts[0].s, f.parts[0].e), (5, 8));
    assert_eq!(f.peak, 6);
    assert_eq!((f.parts[1].s, f.parts[1].e), (5, 7));
    assert_eq!((f.ctx.up, f.ctx.dn), (Some(4), Some(9)));
    assert_eq!((f.pairs[0].a, f.pairs[0].b), (5, 7));
    assert_eq!(f.summary, "sommet pt 7 · 1 paire · écart 40°");
}

/// Invariant C6 — un finding `corrected` n'est pas retraduit, même si les
/// points de son emprise ont disparu ; le `pending` voisin l'est (D5 partiel).
#[test]
fn test_sync_indexes_skips_corrected() {
    let pts = make_points(20);

    // Emprise 5..8 : la suppression des id 5..8 la rend intraduisible.
    let mut corrected = ar_finding("ar-1", &pts, 5, 8, 6, &[(5, 7, 40.0)]);
    corrected.status = FindingStatus::Corrected;
    corrected.parts[0].text = "texte figé".to_string();
    corrected.summary = "sommet pt 7 · 1 paire · écart 40°".to_string();

    let pending = ar_finding("ar-2", &pts, 12, 14, 13, &[(12, 14, 6.5)]);

    let mut findings = vec![corrected.clone(), pending];
    let working = delete_range(&pts, 4, 7); // retire les id 5, 6, 7, 8

    sync_indexes(&working, &mut findings);

    // Le corrigé est intact, point pour point.
    let c = findings
        .iter()
        .find(|f| f.id == "ar-1")
        .expect("finding corrigé conservé");
    assert_eq!(c.status, FindingStatus::Corrected);
    assert_eq!((c.parts[0].s, c.parts[0].e, c.peak), (5, 8, 6));
    assert_eq!(c.parts[0].text, "texte figé");
    assert_eq!(c.summary, "sommet pt 7 · 1 paire · écart 40°");
    assert_eq!((c.pairs[0].a, c.pairs[0].b), (5, 7));

    // Le pending est translaté de 4 rangs (12..14 → 8..10).
    let p = findings
        .iter()
        .find(|f| f.id == "ar-2")
        .expect("finding pending conservé");
    assert_eq!((p.parts[0].s, p.parts[0].e, p.peak), (8, 10, 9));
    assert_eq!((p.pairs[0].a, p.pairs[0].b), (8, 10));
    assert_eq!(p.summary, "sommet pt 10 · 1 paire · écart 40°");
    assert_eq!(p.parts[0].text, "paires miroirs : pts 9↔11 (6.5 m)");
}

/// §2.4 — les textes affichables sont dérivables : après réalignement ils
/// portent les numéros courants, y compris pour un RP (emprise + cœur).
#[test]
fn test_sync_indexes_rebuild_texts() {
    let pts = make_points(30);
    let mut findings = vec![rp_finding("rp-1", &pts, 8, 20, 12, 18, &[(10, 17, 3.5)], 270.0)];

    // Marqueur de texte périmé : il doit être intégralement régénéré.
    findings[0].summary = "jonction pt 13 · 270° (3/4 de tour) · 1 paire".to_string();
    findings[0].parts[0].text = "superpositions : pts 11↔18 (3.5 m)".to_string();

    let working = delete_range(&pts, 0, 2); // 3 points amont retirés
    sync_indexes(&working, &mut findings);

    let f = &findings[0];
    assert_eq!((f.parts[0].s, f.parts[0].e), (5, 17));
    assert_eq!(f.peak, 9);
    assert_eq!((f.parts[1].s, f.parts[1].e), (9, 15), "cœur translaté");
    assert_eq!(f.summary, "jonction pt 10 · 270° (3/4 de tour) · 1 paire");
    assert_eq!(f.parts[0].text, "superpositions : pts 8↔15 (3.5 m)");
    assert_eq!(f.parts[1].text, "cœur : angle cumulé 270° (3/4 de tour)");
}

// ─── Sous-étape 3.1 — rebuild_texts ───────────────────────────────────

/// Gabarits AR (§2.4) : sommet, pluriel des paires, écart arrondi, paires
/// miroirs avec distance à une décimale en deçà de 10 m. Le texte de cœur
/// (sans numéro) n'est pas retouché, comme dans le JS.
#[test]
fn test_rebuild_texts_ar() {
    let pts = make_points(20);
    let mut findings = vec![
        ar_finding("ar-1", &pts, 5, 8, 6, &[(5, 7, 4.0)]),
        ar_finding("ar-2", &pts, 12, 14, 13, &[]),
    ];
    for f in findings.iter_mut() {
        f.parts[1].text = "cœur : demi-tour · branches 12/9 m".to_string();
    }

    sync_indexes(&pts, &mut findings);

    // Une paire, distance < 10 m → une décimale.
    assert_eq!(findings[0].summary, "sommet pt 7 · 1 paire · écart 40°");
    assert_eq!(findings[0].parts[0].text, "paires miroirs : pts 6↔8 (4.0 m)");
    assert_eq!(
        findings[0].parts[1].text, "cœur : demi-tour · branches 12/9 m",
        "le cœur AR ne porte aucun numéro : il n'est pas régénéré"
    );

    // Aucune paire → texte de repli, pluriel au singulier.
    assert_eq!(findings[1].summary, "sommet pt 14 · 0 paire · écart 40°");
    assert_eq!(
        findings[1].parts[0].text,
        "retournement isolé, aucune paire miroir"
    );
}

/// Gabarits RP (§2.4) : jonction, angle cumulé, libellé de rotation, paires de
/// superposition — et repli « refermeture par croisement de trace » sans paire.
#[test]
fn test_rebuild_texts_rp() {
    let pts = make_points(30);
    let mut findings = vec![
        rp_finding("rp-1", &pts, 8, 20, 12, 18, &[(10, 17, 3.5), (11, 16, 40.0)], 270.0),
        rp_finding("rp-2", &pts, 22, 28, 24, 27, &[], 720.0),
    ];

    sync_indexes(&pts, &mut findings);

    assert_eq!(
        findings[0].summary,
        "jonction pt 13 · 270° (3/4 de tour) · 2 paires"
    );
    assert_eq!(
        findings[0].parts[0].text,
        "superpositions : pts 11↔18 (3.5 m) · pts 12↔17 (40 m)"
    );
    assert_eq!(
        findings[0].parts[1].text,
        "cœur : angle cumulé 270° (3/4 de tour)"
    );

    assert_eq!(
        findings[1].summary,
        "jonction pt 25 · 720° (2 tours complets) · 0 paire"
    );
    assert_eq!(
        findings[1].parts[0].text, "refermeture par croisement de trace",
        "sans paire de superposition, le RP signale la refermeture"
    );
}

// ─── Sous-étape 3.1 — nesting_guard ──────────────────────────────────

/// §9.1 — la garde bloque sur un `pending` tiers dont l'emprise intersecte la
/// plage (bornes incluses), ignore le finding courant, les `fp`, les
/// `corrected`, et ne bloque pas sur une plage vide.
#[test]
fn test_nesting_guard_refuses() {
    let pts = make_points(30);
    let mut fp = rp_finding("rp-1", &pts, 6, 7, 6, 7, &[], 300.0);
    fp.status = FindingStatus::Fp;
    let mut corrected = ar_finding("ar-3", &pts, 6, 7, 6, &[]);
    corrected.status = FindingStatus::Corrected;

    let findings = vec![
        ar_finding("ar-1", &pts, 5, 8, 6, &[(5, 7, 40.0)]),
        ar_finding("ar-2", &pts, 20, 24, 22, &[(20, 24, 8.0)]),
        fp,
        corrected,
    ];

    // Refus : la plage [6, 7] mord l'emprise 5..8 de ar-1.
    assert_eq!(
        nesting_guard(&findings, "ar-2", 6, 7).as_deref(),
        Some("ar-1")
    );
    // Intersection à la borne : e de ar-1 (8) ≥ zs (8).
    assert_eq!(
        nesting_guard(&findings, "ar-2", 8, 8).as_deref(),
        Some("ar-1")
    );
    // Le finding courant ne se bloque pas lui-même ; le fp et le corrigé
    // présents dans la plage ne bloquent pas.
    assert_eq!(nesting_guard(&findings, "ar-1", 5, 8), None);
    // Plage vide.
    assert_eq!(nesting_guard(&findings, "ar-2", 9, 8), None);
    // Emprise disjointes.
    assert_eq!(nesting_guard(&findings, "ar-1", 12, 15), None);
}

/// §9.3 — un faux positif ne bloque **jamais** la correction d'une autre
/// anomalie (il est absorbé) ; un `corrected` non plus.
#[test]
fn test_nesting_guard_allows_fp() {
    let pts = make_points(30);
    let mut fp = ar_finding("ar-1", &pts, 5, 8, 6, &[(5, 7, 40.0)]);
    fp.status = FindingStatus::Fp;

    let findings = vec![fp, ar_finding("ar-2", &pts, 20, 24, 22, &[])];

    assert_eq!(
        nesting_guard(&findings, "ar-2", 5, 8),
        None,
        "un faux positif n'est pas un obstacle"
    );

    // Corrigé : même conclusion.
    let mut corrected = ar_finding("ar-3", &pts, 12, 14, 13, &[]);
    corrected.status = FindingStatus::Corrected;
    let findings2 = vec![corrected, ar_finding("ar-2", &pts, 20, 24, 22, &[])];
    assert_eq!(nesting_guard(&findings2, "ar-2", 12, 14), None);
}

// ─── Sous-étape 3.1 — absorb_fp_findings ─────────────────────────────

/// §9.3 — seuls les `fp` dont l'emprise intersecte la plage sont retirés de la
/// liste et retournés (statut `fp` conservé, invariant C9) ; les autres
/// findings — y compris le courant et les `pending` recouverts — restent.
#[test]
fn test_absorb_fp_findings() {
    let pts = make_points(30);
    let mut fp_in = ar_finding("ar-fp1", &pts, 6, 7, 6, &[]);
    fp_in.status = FindingStatus::Fp;
    let mut fp_out = ar_finding("ar-fp2", &pts, 20, 24, 22, &[]);
    fp_out.status = FindingStatus::Fp;
    let mut self_fp = ar_finding("ar-1", &pts, 5, 8, 6, &[]);
    self_fp.status = FindingStatus::Fp;

    let mut findings = vec![
        self_fp,
        fp_in,
        fp_out,
        ar_finding("ar-2", &pts, 7, 7, 7, &[]),
    ];

    let absorbed = absorb_fp_findings(&mut findings, "ar-1", 5, 8);

    assert_eq!(absorbed.len(), 1);
    assert_eq!(absorbed[0].id, "ar-fp1");
    assert_eq!(
        absorbed[0].status,
        FindingStatus::Fp,
        "l'absorbé conserve son statut pour la restitution (C9)"
    );
    assert_eq!(
        ids_of(&findings),
        vec!["ar-1", "ar-fp2", "ar-2"],
        "le courant et le fp hors plage sont conservés, le pending recouvert aussi"
    );

    // Plage vide → aucune absorption, liste intacte.
    let before = ids_of(&findings);
    let absorbed2 = absorb_fp_findings(&mut findings, "ar-1", 9, 8);
    assert!(absorbed2.is_empty());
    assert_eq!(ids_of(&findings), before);
}

// ─── Sous-étape 3.1 — after_trace_edit ───────────────────────────────

/// §4 — la chaîne post-édition réaligne et re-trie les findings par position
/// croissante ; à positions égales, l'ordre de publication est conservé.
#[test]
fn test_after_trace_edit_sorts_by_position() {
    let pts = make_points(30);
    let mut findings = vec![
        ar_finding("ar-3", &pts, 20, 24, 22, &[]),
        ar_finding("ar-1", &pts, 5, 8, 6, &[]),
        rp_finding("rp-2", &pts, 12, 18, 14, 16, &[], 270.0),
    ];

    after_trace_edit(&pts, &mut findings);

    assert_eq!(ids_of(&findings), vec!["ar-1", "rp-2", "ar-3"]);
    assert_eq!(findings[0].parts[0].s, 5);
    assert_eq!(findings[1].parts[0].s, 12);
    assert_eq!(findings[2].parts[0].s, 20);

    // Emprises identiques : ordre stable conservé (comme `Array.sort`).
    let mut tied = vec![
        rp_finding("rp-1", &pts, 5, 8, 6, 7, &[], 270.0),
        ar_finding("ar-1", &pts, 5, 8, 6, &[]),
    ];
    after_trace_edit(&pts, &mut tied);
    assert_eq!(ids_of(&tied), vec!["rp-1", "ar-1"]);
}

// ─── Sous-étape 3.2 — apply_delete ────────────────────────────────────

/// §5.3 — suppression d'une plage médiane : points retirés dans l'ordre, finding
/// marqué `corrected` avec son instantané, index des autres findings traduits.
#[test]
fn test_apply_delete_basic() {
    let pts = make_points(20);
    let findings = vec![
        ar_finding("ar-1", &pts, 5, 8, 6, &[(5, 7, 40.0)]),
        ar_finding("ar-2", &pts, 12, 14, 13, &[(12, 14, 6.5)]),
    ];

    // Suppression de la plage [5..=7] (ids 6, 7, 8) de l'emprise 5..8.
    let state = apply_delete(pts, findings, "ar-1", 5, 7, 100).expect("suppression acceptée");

    let mut expected: Vec<u32> = (1..=5).collect();
    expected.extend(9..=20);
    assert_eq!(pt_ids(&state.points), expected, "plage retirée, ordre conservé");

    let corrected = state
        .findings
        .iter()
        .find(|f| f.id == "ar-1")
        .expect("finding corrigé présent");
    assert_eq!(corrected.status, FindingStatus::Corrected);
    assert_eq!(corrected.correction, Some(CorrectionType::Delete));
    let undo = undo_delete_of(corrected);
    assert_eq!(pt_ids(&undo.orig_pts), vec![6, 7, 8]);
    assert_eq!(undo.anchor_left_id, Some(5));
    assert_eq!(undo.anchor_right_id, Some(9));
    assert_eq!(undo.first_no, 6);
    assert!(undo.absorbed_fp.is_empty());

    // Le corrigé n'est pas retraduit (C6) ; le voisin aval descend de 3 rangs.
    assert_eq!((corrected.parts[0].s, corrected.parts[0].e), (5, 8));
    let downstream = state
        .findings
        .iter()
        .find(|f| f.id == "ar-2")
        .expect("finding aval présent");
    assert_eq!(downstream.status, FindingStatus::Pending);
    assert_eq!(
        (downstream.parts[0].s, downstream.parts[0].e, downstream.peak),
        (9, 11, 10)
    );
    assert_eq!((downstream.pairs[0].a, downstream.pairs[0].b), (9, 11));
    assert_eq!(downstream.summary, "sommet pt 11 · 1 paire · écart 40°");
}

/// §9.1/§5.2 — la garde d'imbrication refuse **avant** la garde de
/// dégénérescence, et nomme l'anomalie bloquante (invariant C2).
#[test]
fn test_apply_delete_nesting_refused() {
    let pts = make_points(20);
    let findings = vec![
        ar_finding("ar-1", &pts, 5, 8, 6, &[(5, 7, 40.0)]),
        ar_finding("ar-2", &pts, 7, 12, 9, &[]),
    ];
    let err = apply_delete(pts, findings, "ar-1", 5, 7, 100).expect_err("imbrication refusée");
    assert!(err.contains("ar-2"), "l'anomalie bloquante est nommée : {err}");
    assert!(err.contains("refusée"), "message de refus attendu : {err}");

    // Priorité des gardes : trace de 4 points où la suppression (0..=3)
    // dégénérerait *et* mord une anomalie pending -> c'est l'imbrication qui
    // tranche (elle est évaluée en premier).
    let small = make_points(4);
    let findings2 = vec![
        ar_finding("ar-1", &small, 0, 3, 1, &[]),
        ar_finding("ar-2", &small, 1, 2, 1, &[]),
    ];
    let err2 = apply_delete(small, findings2, "ar-1", 0, 3, 100)
        .expect_err("imbrication prioritaire sur dégénérescence");
    assert!(err2.contains("ar-2"), "message d'imbrication attendu : {err2}");
    assert!(!err2.contains("dégénérée"), "la dégénérescence n'est pas évaluée : {err2}");
}

/// D10 / C3 — la trace ne descend jamais sous 2 points ; à 2 points exactement,
/// la suppression est acceptée (borne du garde).
#[test]
fn test_apply_delete_degenerate_refused() {
    let pts = make_points(5);
    let findings = vec![ar_finding("ar-1", &pts, 0, 3, 1, &[])];
    let err = apply_delete(pts, findings, "ar-1", 0, 3, 100).expect_err("trace dégénérée");
    assert!(err.contains("dégénérée"), "message attendu : {err}");

    // Borne acceptée : 5 − 3 = 2 points restants.
    let pts2 = make_points(5);
    let findings2 = vec![ar_finding("ar-1", &pts2, 0, 3, 1, &[])];
    let state = apply_delete(pts2, findings2, "ar-1", 0, 2, 100).expect("2 points restants");
    assert_eq!(pt_ids(&state.points), vec![4, 5]);
}

/// D9 — plage vide ou inversée (cas RP `fin = début + 1`) : refus explicite
/// avant tout calcul d'amplitude. Le refus nominal est produit par la commande
/// (`audit_apply_delete`, Phase 4) ; ce test verrouille le garde défensif du
/// moteur, qui évite tout débordement d'entier sur `de − ds + 1`.
#[test]
fn test_apply_delete_empty_range_refused() {
    let pts = make_points(12);
    let findings = vec![ar_finding("ar-1", &pts, 5, 8, 6, &[])];
    let err = apply_delete(pts, findings, "ar-1", 6, 5, 100).expect_err("plage vide refusée");
    assert!(
        err.contains("Aucun point à supprimer entre les curseurs"),
        "message attendu : {err}"
    );
}

/// §11 — suppression d'un point isolé (AR, `ds = de`) : instantané d'un seul
/// point et raccord direct entre les voisins conservés.
#[test]
fn test_apply_delete_single_point() {
    let pts = make_points(10);
    let findings = vec![ar_finding("ar-1", &pts, 2, 6, 4, &[])];

    let state = apply_delete(pts, findings, "ar-1", 4, 4, 100).expect("point isolé supprimé");

    assert_eq!(state.points.len(), 9);
    assert_eq!(pt_ids(&state.points), vec![1, 2, 3, 4, 6, 7, 8, 9, 10]);
    let corrected = state.findings.iter().find(|f| f.id == "ar-1").unwrap();
    let undo = undo_delete_of(corrected);
    assert_eq!(pt_ids(&undo.orig_pts), vec![5]);
    assert_eq!(undo.anchor_left_id, Some(4));
    assert_eq!(undo.anchor_right_id, Some(6));
    assert_eq!(undo.first_no, 5);
}

/// D4 / C7 / C8 — l'instantané est **complet** : copies fidèles des points
/// supprimés (id, lat, lon, ele), ancres, numérotation d'origine.
#[test]
fn test_apply_delete_undo_complete() {
    let pts = make_points(12);
    let findings = vec![ar_finding("ar-1", &pts, 3, 7, 5, &[])];

    let state = apply_delete(pts.clone(), findings, "ar-1", 4, 6, 100).expect("suppression");

    let corrected = state.findings.iter().find(|f| f.id == "ar-1").unwrap();
    let undo = undo_delete_of(corrected);
    assert_eq!(undo.orig_pts.len(), 3);
    for (k, p) in undo.orig_pts.iter().enumerate() {
        let src = &pts[4 + k];
        assert_eq!(p.id, src.id);
        assert_eq!(p.lat, src.lat);
        assert_eq!(p.lon, src.lon);
        assert_eq!(p.ele, src.ele);
    }
    assert_eq!(undo.first_no, 5);
    // La trace résultante est la trace d'origine privée de la plage (C8).
    let mut expected = pt_ids(&pts);
    expected.drain(4..=6);
    assert_eq!(pt_ids(&state.points), expected);
}

/// §11 — anomalie en tête de trace : `anchor_left_id` nul, réinsertion par
/// l'ancre droite seule.
#[test]
fn test_apply_delete_anchor_left_null() {
    let pts = make_points(10);
    let findings = vec![ar_finding("ar-1", &pts, 0, 3, 1, &[])];

    let state = apply_delete(pts, findings, "ar-1", 0, 1, 100).expect("tête supprimée");

    let corrected = state.findings.iter().find(|f| f.id == "ar-1").unwrap();
    let undo = undo_delete_of(corrected);
    assert_eq!(undo.anchor_left_id, None, "aucun point conservé en amont");
    assert_eq!(undo.anchor_right_id, Some(3));
    assert_eq!(pt_ids(&undo.orig_pts), vec![1, 2]);
    assert_eq!(pt_ids(&state.points), (3..=10).collect::<Vec<u32>>());
}

/// §11 — plage touchant la fin de trace : `anchor_right_id` nul, réinsertion en
/// queue par l'ancre gauche.
#[test]
fn test_apply_delete_anchor_right_null() {
    let pts = make_points(10);
    let findings = vec![ar_finding("ar-1", &pts, 5, 9, 7, &[])];

    let state = apply_delete(pts, findings, "ar-1", 8, 9, 100).expect("queue supprimée");

    let corrected = state.findings.iter().find(|f| f.id == "ar-1").unwrap();
    let undo = undo_delete_of(corrected);
    assert_eq!(undo.anchor_left_id, Some(8));
    assert_eq!(undo.anchor_right_id, None, "aucun point conservé en aval");
    assert_eq!(pt_ids(&undo.orig_pts), vec![9, 10]);
    assert_eq!(pt_ids(&state.points), (1..=8).collect::<Vec<u32>>());
}

/// D6 / §9.3 / C9 — un faux positif recouvert par la suppression est absorbé
/// (retiré de la liste, conservé dans l'instantané avec son statut) ; un faux
/// positif hors plage n'est pas touché.
#[test]
fn test_apply_delete_absorbs_fp() {
    let pts = make_points(20);
    let mut fp_in = ar_finding("ar-fp1", &pts, 6, 7, 6, &[]);
    fp_in.status = FindingStatus::Fp;
    let mut fp_out = ar_finding("ar-fp2", &pts, 15, 16, 15, &[]);
    fp_out.status = FindingStatus::Fp;

    let findings = vec![
        ar_finding("ar-1", &pts, 5, 8, 6, &[(5, 7, 40.0)]),
        fp_in,
        fp_out,
        ar_finding("ar-2", &pts, 12, 14, 13, &[]),
    ];

    let state = apply_delete(pts, findings, "ar-1", 5, 7, 100).expect("suppression avec absorption");

    // §3.1 — la liste est re-triée par position : le pending aval (indices 9..11
    // après suppression) précède le faux positif conservé (indices 12..13).
    assert_eq!(
        ids_of(&state.findings),
        vec!["ar-1", "ar-2", "ar-fp2"],
        "le faux positif recouvert quitte la liste, les autres restent"
    );
    let corrected = state.findings.iter().find(|f| f.id == "ar-1").unwrap();
    let undo = undo_delete_of(corrected);
    assert_eq!(undo.absorbed_fp.len(), 1);
    assert_eq!(undo.absorbed_fp[0].id, "ar-fp1");
    assert_eq!(
        undo.absorbed_fp[0].status,
        FindingStatus::Fp,
        "statut conservé pour la restitution (C9)"
    );
}

/// C12 — un finding déjà `corrected` ne peut être re-corrigé (le JS refuse
/// silencieusement toute correction hors statut `pending`), et un finding inconnu
/// est refusé : dans les deux cas, aucun état n'est modifié.
#[test]
fn test_apply_delete_rejects_non_pending() {
    let pts = make_points(20);
    let mut corrected = ar_finding("ar-1", &pts, 5, 8, 6, &[(5, 7, 40.0)]);
    corrected.status = FindingStatus::Corrected;
    let mut fp = ar_finding("ar-2", &pts, 12, 14, 13, &[]);
    fp.status = FindingStatus::Fp;

    let err = apply_delete(pts.clone(), vec![corrected.clone(), fp.clone()], "ar-1", 5, 7, 100)
        .expect_err("refus d'une anomalie déjà corrigée");
    assert!(err.contains("déjà traitée"), "message attendu : {err}");

    // Le faux positif n'est pas davantage corrigeable par ce chemin (§7 : le
    // retrait du marqueur est une opération distincte).
    let err2 = apply_delete(pts.clone(), vec![corrected, fp], "ar-2", 12, 14, 100)
        .expect_err("refus d'un faux positif");
    assert!(err2.contains("déjà traitée"), "message attendu : {err2}");

    // Anomalie inconnue : refus également.
    let findings = vec![ar_finding("ar-1", &pts, 5, 8, 6, &[])];
    let err3 =
        apply_delete(pts, findings, "ar-9", 5, 7, 100).expect_err("refus d'une anomalie inconnue");
    assert!(err3.contains("inconnue"), "message attendu : {err3}");
}

// ─── Sous-étape 3.3 — apply_route ─────────────────────────────────────

/// Tracé ORS factice : `n` points alignés vers le nord-est.
fn make_coords(n: usize) -> Vec<LatLon> {
    (0..n)
        .map(|k| LatLon {
            lat: 46.0 + (k as f64) * 0.001,
            lon: 4.0 + (k as f64) * 0.001,
        })
        .collect()
}

/// Extrait l'instantané d'annulation d'un routage, en vérifiant son type.
fn undo_route_of(f: &Finding) -> &UndoRoute {
    match f.undo.as_ref() {
        Some(UndoRecord::Route(u)) => u,
        other => panic!("undo attendu de type route, obtenu {:?}", other),
    }
}

/// Compare deux traces point par point — id, coordonnées, élévation et ordre
/// (invariant C8 : restitution exacte du multi-ensemble).
fn assert_points_eq(actual: &[AuditPoint], expected: &[AuditPoint]) {
    assert_eq!(actual.len(), expected.len(), "nombre de points");
    for (k, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            (a.id, a.lat, a.lon, a.ele),
            (e.id, e.lat, e.lon, e.ele),
            "point d'index {k}"
        );
    }
}

/// D5 — état du scénario imbriqué (AR dans l'emprise de la RP) après les deux
/// corrections.
///
/// Configuration réelle documentée (§9) : un zigzag AR dans l'approche d'une
/// boucle RP. La correction de la RP porte sur son **cœur** — une plage mordant
/// l'AR `pending` serait refusée par la garde d'imbrication (§9.1) — puis l'AR
/// est corrigée à son tour, hors de toute emprise `pending`.
fn nested_corrected_state() -> AuditState {
    let pts = make_points(20);
    let findings = vec![
        ar_finding("ar-1", &pts, 5, 7, 6, &[(5, 7, 40.0)]),             // approche
        rp_finding("rp-1", &pts, 4, 14, 8, 12, &[(6, 13, 3.0)], 300.0), // boucle
    ];

    // Une plage de RP morduant l'AR pending est refusée…
    let err = apply_delete(pts.clone(), findings.clone(), "rp-1", 5, 6, 100)
        .expect_err("plage morduant l'AR pending : refus attendu");
    assert!(err.contains("ar-1"), "l'anomalie bloquante est l'AR : {err}");

    // …alors que la correction du cœur (hors emprise de l'AR) est acceptée.
    let state = apply_delete(pts, findings, "rp-1", 9, 11, 100).expect("suppression du cœur RP");
    apply_delete(state.points, state.findings, "ar-1", 6, 6, 100).expect("suppression AR")
}

/// D5 — après les deux annulations, les findings portent à nouveau leurs index
/// d'origine et sont `pending` sans instantané.
fn assert_d5_findings_pending(findings: &[Finding]) {
    let ar = findings.iter().find(|f| f.id == "ar-1").expect("ar-1");
    assert_eq!(ar.status, FindingStatus::Pending);
    assert_eq!(ar.correction, None);
    assert!(ar.undo.is_none(), "instantané consommé");
    assert_eq!((ar.parts[0].s, ar.parts[0].e, ar.peak), (5, 7, 6));

    let rp = findings.iter().find(|f| f.id == "rp-1").expect("rp-1");
    assert_eq!(rp.status, FindingStatus::Pending);
    assert_eq!(rp.correction, None);
    assert!(rp.undo.is_none(), "instantané consommé");
    assert_eq!((rp.parts[0].s, rp.parts[0].e), (4, 14));
    assert_eq!((rp.parts[1].s, rp.parts[1].e), (8, 12));
}

/// §6.3 — l'intérieur est remplacé par les points ORS (ids neufs), les ancres
/// sont conservées telles quelles, le finding est marqué et les index aval sont
/// retraduits.
#[test]
fn test_apply_route_basic() {
    let pts = make_points(20);
    let findings = vec![
        ar_finding("ar-1", &pts, 5, 10, 7, &[]),
        ar_finding("ar-2", &pts, 12, 14, 13, &[(12, 14, 6.5)]),
    ];
    let coords = make_coords(4); // 2 points insérés : coords[1..3]

    // Ancres : points[6] (id 7) et points[8] (id 9) — intérieur = id 8.
    let state = apply_route(pts, findings, "ar-1", 6, 8, coords.clone(), "driving-car", 100)
        .expect("routage accepté");

    let mut expected: Vec<u32> = (1..=7).collect();
    expected.extend([100, 101]);
    expected.extend(9..=20);
    assert_eq!(pt_ids(&state.points), expected, "mids insérés entre les ancres");

    let corrected = state
        .findings
        .iter()
        .find(|f| f.id == "ar-1")
        .expect("finding corrigé présent");
    assert_eq!(corrected.status, FindingStatus::Corrected);
    assert_eq!(corrected.correction, Some(CorrectionType::RouteCar));

    let undo = undo_route_of(corrected);
    assert_eq!(pt_ids(&undo.orig_pts), vec![8], "points remplacés copiés (C7)");
    assert_eq!(undo.inserted_ids, vec![100, 101], "ordre amont → aval (§6.4)");
    assert_eq!(undo.first_no, 8);
    assert!(undo.absorbed_fp.is_empty());
    assert_eq!(undo.start_pt.lat, 45.007);
    assert_eq!(undo.end_pt.lat, 45.009);

    // Les ancres sont conservées avec leurs coordonnées et leur élévation.
    let anchor_start = state.points.iter().find(|p| p.id == 7).unwrap();
    assert_eq!((anchor_start.lat, anchor_start.ele), (45.007, Some(100.0)));
    let anchor_end = state.points.iter().find(|p| p.id == 9).unwrap();
    assert_eq!((anchor_end.lat, anchor_end.ele), (45.009, Some(100.0)));

    // Les points insérés ne portent pas d'élévation (LatLon sans `ele`).
    let mid = state.points.iter().find(|p| p.id == 100).unwrap();
    assert_eq!((mid.lat, mid.lon, mid.ele), (46.001, 4.001, None));

    // Le finding aval est translaté du solde net (+1 point).
    let downstream = state.findings.iter().find(|f| f.id == "ar-2").unwrap();
    assert_eq!(
        (downstream.parts[0].s, downstream.parts[0].e, downstream.peak),
        (13, 15, 14)
    );
}

/// §6.1 — seuls `driving-car` et `cycling-road` sont admis ; rien n'est modifié
/// si le profil est refusé.
#[test]
fn test_apply_route_profile_invalid() {
    let pts = make_points(20);
    let findings = vec![ar_finding("ar-1", &pts, 5, 10, 7, &[])];

    let err = apply_route(pts, findings, "ar-1", 6, 8, make_coords(3), "walking", 100)
        .expect_err("profil refusé");
    assert!(err.contains("Profil"), "message attendu : {err}");
}

/// §6.3 — un tracé de moins de 2 points est refusé ; à 2 points exactement,
/// `mids` est vide (aucun point inséré) mais la correction reste appliquée,
/// comme dans le JS.
#[test]
fn test_apply_route_coords_too_short() {
    let pts = make_points(20);
    let findings = vec![ar_finding("ar-1", &pts, 5, 10, 7, &[])];
    let err = apply_route(pts, findings, "ar-1", 6, 8, make_coords(1), "driving-car", 100)
        .expect_err("tracé trop court");
    assert!(
        err.contains("Tracé ORS invalide (moins de 2 points)."),
        "message attendu : {err}"
    );

    // Borne acceptée : 2 coordonnées = 0 point inséré (l'intérieur est retiré).
    let pts2 = make_points(20);
    let findings2 = vec![ar_finding("ar-1", &pts2, 5, 10, 7, &[])];
    let state = apply_route(pts2, findings2, "ar-1", 6, 8, make_coords(2), "driving-car", 100)
        .expect("routage à mids vide");
    let corrected = state.findings.iter().find(|f| f.id == "ar-1").unwrap();
    assert_eq!(undo_route_of(corrected).inserted_ids, Vec::<u32>::new());
    assert!(state.points.iter().all(|p| p.id <= 20), "aucun id alloué");
}

/// D7 / §9.1 — l'intérieur du routage ne peut pas mordre un `pending` tiers :
/// la garde le refuse et nomme l'anomalie bloquante (invariant C2).
#[test]
fn test_apply_route_nesting_refused() {
    let pts = make_points(20);
    let findings = vec![
        ar_finding("ar-1", &pts, 5, 10, 7, &[]),
        ar_finding("ar-2", &pts, 7, 8, 7, &[]),
    ];
    let err = apply_route(pts, findings, "ar-1", 6, 8, make_coords(4), "driving-car", 100)
        .expect_err("imbrication refusée");
    assert!(err.contains("ar-2"), "l'anomalie bloquante est nommée : {err}");
}

/// §6.4 / §11 — ancres adjacentes : `inner` vide, les points routés s'insèrent
/// entre les deux ancres conservées et l'annulation reste possible.
#[test]
fn test_apply_route_empty_inner() {
    let pts = make_points(20);
    let findings = vec![ar_finding("ar-1", &pts, 5, 10, 7, &[])];

    let state = apply_route(pts, findings, "ar-1", 5, 6, make_coords(3), "cycling-road", 100)
        .expect("ancres adjacentes");

    let mut expected: Vec<u32> = (1..=6).collect();
    expected.push(100);
    expected.extend(7..=20);
    assert_eq!(pt_ids(&state.points), expected);

    let corrected = state.findings.iter().find(|f| f.id == "ar-1").unwrap();
    assert_eq!(corrected.correction, Some(CorrectionType::RouteBike));
    let undo = undo_route_of(corrected);
    assert!(undo.orig_pts.is_empty(), "aucun point remplacé");
    assert_eq!(undo.inserted_ids, vec![100]);
    assert_eq!(undo.first_no, 7);
    // Garde-fous triviaux : la séquence insérée est intacte.
    assert!(state.points.iter().any(|p| p.id == 100));
}

/// §2.3 — le tracé de rendu est **préfixé/suffixé par les ancres exactes** :
/// |route_pts| = |coords|, les extrémités étant les ancres de la trace.
#[test]
fn test_apply_route_undo_route_pts() {
    let pts = make_points(20);
    let findings = vec![ar_finding("ar-1", &pts, 5, 10, 7, &[])];
    let coords = make_coords(5);

    let state = apply_route(pts, findings, "ar-1", 6, 9, coords.clone(), "driving-car", 100)
        .expect("routage accepté");

    let corrected = state.findings.iter().find(|f| f.id == "ar-1").unwrap();
    let undo = undo_route_of(corrected);

    assert_eq!(undo.route_pts.len(), coords.len(), "|route_pts| = |coords|");
    assert_eq!(
        undo.inserted_ids.len(),
        coords.len() - 2,
        "|mids| = |coords| − 2 (ancres non dupliquées)"
    );
    // Première et dernière extrémités : ancres exactes de la trace de travail.
    assert_eq!((undo.route_pts[0].lat, undo.route_pts[0].lon), (45.007, 3.0));
    assert_eq!((undo.route_pts[0].lat, undo.route_pts[0].lon), (undo.start_pt.lat, undo.start_pt.lon));
    let last = undo.route_pts.last().unwrap();
    assert_eq!((last.lat, last.lon), (45.01, 3.0));
    assert_eq!((last.lat, last.lon), (undo.end_pt.lat, undo.end_pt.lon));
    // Points intermédiaires : coordonnées ORS internes, dans l'ordre.
    for (k, c) in coords[1..coords.len() - 1].iter().enumerate() {
        assert_eq!((undo.route_pts[k + 1].lat, undo.route_pts[k + 1].lon), (c.lat, c.lon));
    }
}

/// C5 / §6.4 — les points insérés reçoivent des ids alloués depuis
/// `next_point_id`, strictement croissants, sans collision avec les ids vivants.
#[test]
fn test_apply_route_new_ids_allocated() {
    let pts = make_points(20);
    let findings = vec![ar_finding("ar-1", &pts, 5, 10, 7, &[])];

    let state = apply_route(pts, findings, "ar-1", 6, 8, make_coords(5), "driving-car", 1000)
        .expect("routage accepté");

    let corrected = state.findings.iter().find(|f| f.id == "ar-1").unwrap();
    assert_eq!(undo_route_of(corrected).inserted_ids, vec![1000, 1001, 1002]);

    let inserted: Vec<u32> = state
        .points
        .iter()
        .map(|p| p.id)
        .filter(|id| *id >= 1000)
        .collect();
    assert_eq!(inserted, vec![1000, 1001, 1002], "ordre amont → aval");
    // Aucun id existant n'est recyclé : les 20 points d'origine restent, moins
    // l'intérieur remplacé (l'id 8).
    assert_eq!(
        state.points.iter().filter(|p| p.id <= 20).count(),
        19,
        "les 20 points d'origine moins l'intérieur remplacé"
    );
}

/// D6 / §9.3 / C9 — un faux positif dont l'emprise est recouverte par le routage
/// est absorbé et conservé dans l'instantané avec son statut.
#[test]
fn test_apply_route_absorbs_fp() {
    let pts = make_points(20);
    let mut fp_in = ar_finding("ar-fp1", &pts, 7, 8, 7, &[]);
    fp_in.status = FindingStatus::Fp;
    let mut fp_out = ar_finding("ar-fp2", &pts, 14, 15, 14, &[]);
    fp_out.status = FindingStatus::Fp;

    let findings = vec![
        ar_finding("ar-1", &pts, 5, 10, 7, &[]),
        fp_in,
        fp_out,
        ar_finding("ar-2", &pts, 12, 13, 12, &[]),
    ];

    let state = apply_route(pts, findings, "ar-1", 6, 8, make_coords(4), "driving-car", 100)
        .expect("routage avec absorption");

    assert!(
        !state.findings.iter().any(|f| f.id == "ar-fp1"),
        "le faux positif recouvert quitte la liste"
    );
    let corrected = state.findings.iter().find(|f| f.id == "ar-1").unwrap();
    let undo = undo_route_of(corrected);
    assert_eq!(undo.absorbed_fp.len(), 1);
    assert_eq!(undo.absorbed_fp[0].id, "ar-fp1");
    assert_eq!(undo.absorbed_fp[0].status, FindingStatus::Fp, "C9");

    // Le faux positif hors zone reste en place.
    assert!(state.findings.iter().any(|f| f.id == "ar-fp2"));
}

// ─── Sous-étape 3.4 — faux positif et annulation ──────────────────────

/// §7 — marquer un finding `pending` faux positif ne touche pas la trace :
/// aucun instantané n'est posé, la correction reste nulle.
#[test]
fn test_mark_fp_basic() {
    let pts = make_points(12);
    let findings = vec![
        ar_finding("ar-1", &pts, 5, 8, 6, &[(5, 7, 40.0)]),
        ar_finding("ar-2", &pts, 10, 11, 10, &[]),
    ];

    let out = mark_fp(findings, "ar-1").expect("marquage faux positif");

    let f = out.iter().find(|f| f.id == "ar-1").unwrap();
    assert_eq!(f.status, FindingStatus::Fp);
    assert_eq!(f.correction, None);
    assert!(f.undo.is_none(), "la trace est intacte : aucun instantané");
    assert_eq!(out.iter().find(|f| f.id == "ar-2").unwrap().status, FindingStatus::Pending);
}

/// §7 / C12 — seul un finding `pending` peut être marqué (le JS sort
/// silencieusement pour tout autre statut) ; un id inconnu est refusé.
#[test]
fn test_mark_fp_rejects_corrected() {
    let pts = make_points(12);
    let mut corrected = ar_finding("ar-1", &pts, 5, 8, 6, &[]);
    corrected.status = FindingStatus::Corrected;
    let mut fp = ar_finding("ar-2", &pts, 10, 11, 10, &[]);
    fp.status = FindingStatus::Fp;

    let err = mark_fp(vec![corrected], "ar-1").expect_err("refus d'un corrigé");
    assert!(err.contains("déjà traité"), "message attendu : {err}");

    let err2 = mark_fp(vec![fp], "ar-2").expect_err("refus d'un faux positif");
    assert!(err2.contains("déjà traité"), "message attendu : {err2}");

    let err3 = mark_fp(vec![ar_finding("ar-1", &pts, 5, 8, 6, &[])], "ar-9")
        .expect_err("refus d'un id inconnu");
    assert!(err3.contains("inconnue"), "message attendu : {err3}");
}

/// §7 — le retrait du marqueur rend le finding de nouveau traitable ; il est
/// refusé sur tout autre statut.
#[test]
fn test_unmark_fp_basic() {
    let pts = make_points(12);
    let mut fp = ar_finding("ar-1", &pts, 5, 8, 6, &[]);
    fp.status = FindingStatus::Fp;
    fp.correction = Some(CorrectionType::Delete);

    let out = unmark_fp(vec![fp], "ar-1").expect("retrait du marqueur");
    let f = &out[0];
    assert_eq!(f.status, FindingStatus::Pending);
    assert_eq!(f.correction, None);

    let err = unmark_fp(out, "ar-1").expect_err("refus sur un pending");
    assert!(err.contains("non marqué"), "message attendu : {err}");
}

/// D4 / C8 — suppression puis annulation : la trace est restituée **point par
/// point** (ids, coordonnées, élévation, ordre) et le finding redevient
/// `pending` avec ses index d'origine et son instantané consommé (C12).
#[test]
fn test_undo_delete_anchor_right() {
    let pts = make_points(20);
    let findings = vec![ar_finding("ar-1", &pts, 5, 8, 6, &[(5, 7, 40.0)])];

    let del = apply_delete(pts.clone(), findings, "ar-1", 5, 7, 100).expect("suppression");
    let und = undo_correction(del.points, del.findings, "ar-1").expect("annulation");

    assert_points_eq(&und.points, &pts);
    let f = und.findings.iter().find(|f| f.id == "ar-1").unwrap();
    assert_eq!(f.status, FindingStatus::Pending);
    assert_eq!(f.correction, None);
    assert!(f.undo.is_none(), "instantané consommé une seule fois (C12)");
    assert_eq!((f.parts[0].s, f.parts[0].e, f.peak), (5, 8, 6));
}

/// §8.2 / §11 — plage touchant la fin de trace : la réinsertion se fait par
/// l'ancre gauche, en queue.
#[test]
fn test_undo_delete_anchor_left_only() {
    let pts = make_points(10);
    let findings = vec![ar_finding("ar-1", &pts, 5, 9, 7, &[])];

    let del = apply_delete(pts.clone(), findings, "ar-1", 8, 9, 100).expect("suppression queue");
    let f = del.findings.iter().find(|f| f.id == "ar-1").unwrap();
    let undo = undo_delete_of(f);
    assert_eq!(undo.anchor_right_id, None);
    assert_eq!(undo.anchor_left_id, Some(8));

    let und = undo_correction(del.points, del.findings, "ar-1").expect("annulation");
    assert_points_eq(&und.points, &pts);
}

/// §11 / §8.2 — les deux ancres ont disparu par une correction ultérieure :
/// l'annulation est refusée (elle ne peut replacer la plage).
#[test]
fn test_undo_delete_anchors_lost() {
    let pts = make_points(20);
    let findings = vec![ar_finding("ar-1", &pts, 3, 9, 6, &[(5, 7, 40.0)])];

    // Suppression de la plage [5..=6] (ids 6, 7) : ancres = ids 5 et 8.
    let del = apply_delete(pts, findings, "ar-1", 5, 6, 100).expect("première suppression");
    let f = del.findings.iter().find(|f| f.id == "ar-1").unwrap();
    let undo = undo_delete_of(f);
    assert_eq!((undo.anchor_left_id, undo.anchor_right_id), (Some(5), Some(8)));

    // Les deux ancres disparaissent sous des corrections ultérieures — ce que la
    // garde d'imbrication interdit en temps normal (C2) : la situation est
    // simulée pour vérifier le garde-fou de réinsertion (§8.2, cas limite §11).
    let points: Vec<AuditPoint> = del
        .points
        .iter()
        .filter(|p| p.id != 5 && p.id != 8)
        .cloned()
        .collect();
    assert_eq!(points.len(), del.points.len() - 2);

    let err = undo_correction(points, del.findings, "ar-1")
        .expect_err("annulation refusée : ancres disparues");
    assert!(err.contains("ancrage disparus"), "message attendu : {err}");
}

/// D6 / §8.3 / C9 — un faux positif absorbé par la suppression est réinjecté
/// avec son statut à l'annulation, à sa place et avec ses index retraduits.
#[test]
fn test_undo_delete_restores_fp() {
    let pts = make_points(20);
    let mut fp = ar_finding("ar-fp1", &pts, 6, 7, 6, &[]);
    fp.status = FindingStatus::Fp;
    let findings = vec![ar_finding("ar-1", &pts, 5, 8, 6, &[]), fp];

    let del = apply_delete(pts.clone(), findings, "ar-1", 5, 7, 100).expect("suppression");
    assert!(
        !del.findings.iter().any(|f| f.id == "ar-fp1"),
        "le faux positif est absorbé"
    );

    let und = undo_correction(del.points, del.findings, "ar-1").expect("annulation");
    assert_points_eq(&und.points, &pts);
    let restored = und
        .findings
        .iter()
        .find(|f| f.id == "ar-fp1")
        .expect("faux positif réinjecté");
    assert_eq!(restored.status, FindingStatus::Fp, "statut maintenu (C9)");
    assert_eq!(restored.correction, None);
    assert_eq!((restored.parts[0].s, restored.parts[0].e), (6, 7));
    assert_eq!(restored.summary, "sommet pt 7 · 0 paire · écart 40°");
}

/// D3 / C8 — routage puis annulation : trace identique point par point, points
/// insérés disparus, finding `pending` avec ses index d'origine.
#[test]
fn test_undo_route_basic() {
    let pts = make_points(20);
    let findings = vec![ar_finding("ar-1", &pts, 5, 10, 7, &[])];

    let rout = apply_route(pts.clone(), findings, "ar-1", 6, 8, make_coords(4), "driving-car", 100)
        .expect("routage");
    assert!(rout.points.iter().any(|p| p.id >= 100));

    let und = undo_correction(rout.points, rout.findings, "ar-1").expect("annulation");

    assert_points_eq(&und.points, &pts);
    assert!(und.points.iter().all(|p| p.id < 100), "points routés retirés");
    let f = und.findings.iter().find(|f| f.id == "ar-1").unwrap();
    assert_eq!(f.status, FindingStatus::Pending);
    assert_eq!(f.correction, None);
    assert!(f.undo.is_none());
    assert_eq!((f.parts[0].s, f.parts[0].e, f.peak), (5, 10, 7));
}

/// D8 / §8.1 garde-fou 1 — un point routé devenu l'origine d'une correction
/// ultérieure empêche l'annulation du routage ; la correction ultérieure reste
/// annulable elle.
#[test]
fn test_undo_route_used_by_other() {
    let pts = make_points(20);
    let findings = vec![
        ar_finding("ar-1", &pts, 5, 10, 7, &[]),
        ar_finding("ar-2", &pts, 12, 14, 13, &[]),
    ];

    let rout = apply_route(pts, findings, "ar-1", 6, 8, make_coords(5), "driving-car", 100)
        .expect("routage"); // insère les ids 100, 101, 102
    // Suppression ultérieure qui place deux points routés dans ses origines.
    let del = apply_delete(rout.points, rout.findings, "ar-2", 7, 8, 200)
        .expect("suppression chevauchante");
    let other = del.findings.iter().find(|f| f.id == "ar-2").unwrap();
    let other_undo = undo_delete_of(other);
    assert_eq!(pt_ids(&other_undo.orig_pts), vec![100, 101]);

    let err = undo_correction(del.points.clone(), del.findings.clone(), "ar-1")
        .expect_err("annulation du routage refusée");
    assert!(err.contains("réutilisée"), "message attendu : {err}");

    // La correction ultérieure, elle, reste annulable.
    let und = undo_correction(del.points, del.findings, "ar-2").expect("annulation de la suppression");
    assert!(und.points.iter().any(|p| p.id == 101));
}

/// D8 / §8.1 garde-fou 3 — séquence insérée rompue (état incohérent qui ne
/// devrait pas survenir si C2/C5 sont maintenus) : l'annulation du routage est
/// refusée plutôt que de produire une trace fausse.
#[test]
fn test_undo_route_contiguity_broken() {
    let pts = make_points(20);
    let findings = vec![ar_finding("ar-1", &pts, 5, 10, 7, &[])];
    let rout = apply_route(pts, findings, "ar-1", 6, 8, make_coords(5), "driving-car", 100)
        .expect("routage"); // insère les ids 100, 101, 102

    // Le deuxième point inséré disparaît : la séquence n'est plus contiguë.
    let mut points = rout.points.clone();
    let before = points.len();
    points.retain(|p| p.id != 101);
    assert_eq!(points.len(), before - 1);

    let err = undo_correction(points, rout.findings, "ar-1")
        .expect_err("annulation refusée : séquence rompue");
    assert!(
        err.contains("modifiée par une correction ultérieure"),
        "message attendu : {err}"
    );
}

/// §8.1 garde-fou 2 — les points insérés ne sont plus dans la trace :
/// annulation refusée. Cas limite inclus : routage à `mids` vide (aucun id
/// inséré), dont l'annulation est structurellement impossible.
#[test]
fn test_undo_route_pos_not_found() {
    let pts = make_points(20);
    let findings = vec![ar_finding("ar-1", &pts, 5, 10, 7, &[])];
    let rout = apply_route(pts.clone(), findings, "ar-1", 6, 8, make_coords(5), "driving-car", 100)
        .expect("routage");

    let mut points = rout.points.clone();
    points.retain(|p| p.id < 100);
    let err = undo_correction(points, rout.findings, "ar-1")
        .expect_err("annulation refusée : points introuvables");
    assert!(err.contains("introuvables"), "message attendu : {err}");

    // Routage à `mids` vide : aucun id inséré, donc aucun point à retrouver.
    let findings2 = vec![ar_finding("ar-1", &pts, 5, 10, 7, &[])];
    let rout2 = apply_route(pts, findings2, "ar-1", 6, 8, make_coords(2), "driving-car", 100)
        .expect("routage sans point inséré");
    let err2 = undo_correction(rout2.points, rout2.findings, "ar-1")
        .expect_err("annulation refusée : mids vide");
    assert!(err2.contains("introuvables"), "message attendu : {err2}");
}

/// D6 / §8.3 / C9 — un faux positif absorbé par le routage est réinjecté avec
/// son statut à l'annulation.
#[test]
fn test_undo_route_restores_fp() {
    let pts = make_points(20);
    let mut fp = ar_finding("ar-fp1", &pts, 7, 8, 7, &[]);
    fp.status = FindingStatus::Fp;
    let findings = vec![ar_finding("ar-1", &pts, 5, 10, 7, &[]), fp];

    let rout = apply_route(pts.clone(), findings, "ar-1", 6, 8, make_coords(4), "driving-car", 100)
        .expect("routage");
    assert!(!rout.findings.iter().any(|f| f.id == "ar-fp1"));

    let und = undo_correction(rout.points, rout.findings, "ar-1").expect("annulation");
    assert_points_eq(&und.points, &pts);
    let restored = und
        .findings
        .iter()
        .find(|f| f.id == "ar-fp1")
        .expect("faux positif réinjecté");
    assert_eq!(restored.status, FindingStatus::Fp);
}

/// D5 — AR imbriquée dans une RP : les deux corrections appliquées, puis les
/// annulations **dans un ordre quelconque** restituent l'état initial ; les deux
/// findings redeviennent `pending` avec leurs index d'origine.
#[test]
fn test_scenario_d5_nested_ar_in_rp_undos() {
    let initial = make_points(20);

    // Ordre 1 — annulation de la RP, puis de l'AR.
    let s = nested_corrected_state();
    let s = undo_correction(s.points, s.findings, "rp-1").expect("annulation RP");
    let s = undo_correction(s.points, s.findings, "ar-1").expect("annulation AR");
    assert_points_eq(&s.points, &initial);
    assert_d5_findings_pending(&s.findings);

    // Ordre 2 — annulation de l'AR, puis de la RP.
    let s = nested_corrected_state();
    let s = undo_correction(s.points, s.findings, "ar-1").expect("annulation AR");
    let s = undo_correction(s.points, s.findings, "rp-1").expect("annulation RP");
    assert_points_eq(&s.points, &initial);
    assert_d5_findings_pending(&s.findings);
}

// ─── Sous-étape 3.5 — migration et export ─────────────────────────────

/// Dossier temporaire propre au test, supprimé en sortie.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!("visugps2_audit_{}_{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("création du dossier temporaire");
        TempDir { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// GPX source minimal mais réaliste : attributs, `<metadata>` complet, trace.
const SOURCE_GPX: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gpx version=\"1.1\" creator=\"Garmin Connect\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n  <metadata>\n    <name>Sortie du dimanche</name>\n    <desc>Trace d'origine</desc>\n    <author><name>JM</name></author>\n    <time>2026-01-01T08:00:00Z</time>\n  </metadata>\n  <trk><name>Ma Trace 2026</name><trkseg>\n    <trkpt lat=\"45.0\" lon=\"3.0\"><ele>100.00</ele></trkpt>\n  </trkseg></trk>\n</gpx>\n";

/// `<metadata>` sérialisé du source — commence par sa balise ouvrante, comme le
/// `XMLSerializer` du JS (contrat d'entrée de `rewrite_gpx`).
const SOURCE_META: &str = "<metadata>\n    <name>Sortie du dimanche</name>\n    <desc>Trace d'origine</desc>\n    <author><name>JM</name></author>\n    <time>2026-01-01T08:00:00Z</time>\n  </metadata>";

/// Attributs de `<gpx>` du source, réémis à l'identique.
const SOURCE_ATTRS: &str = "version=\"1.1\" creator=\"Garmin Connect\" xmlns=\"http://www.topografix.com/GPX/1/1\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"";

/// Écrit un GPX source dans `dir` et retourne son chemin.
fn write_source_gpx(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, body).expect("écriture du GPX source");
    path
}

/// D1 — un registre contenant la clé quoteé `"cleaning_status"` est obsolète.
#[test]
fn test_is_obsolete_registry_true() {
    assert!(is_obsolete_registry(
        r#"[{"id":"a","cleaning_status":"clean"}]"#
    ));
    assert!(is_obsolete_registry(
        r#"{"traces":[{"cleaning_status":"needs_review","name":"x"}]}"#
    ));
}

/// D1 — le format cible (`audit_status`) et un texte quelconque ne sont pas
/// obsolètes : le test porte sur la sous-chaîne **quoteé**.
#[test]
fn test_is_obsolete_registry_false() {
    assert!(!is_obsolete_registry(""));
    assert!(!is_obsolete_registry(
        r#"[{"id":"a","audit_status":"clean","cleaning_phase":""}]"#
    ));
    assert!(
        !is_obsolete_registry("// ancien champ cleaning_status sans guillemets"),
        "une occurrence nue ne déclenche pas la détection"
    );
}

/// D2c — les fichiers de travail de l'ancien module sont supprimés dans tous les
/// dossiers de traces ; le reste (GPX, backup, geojson, keyframes) est conservé,
/// de même que `cleaning.*` hors dossier de trace.
#[test]
fn test_cleanup_obsolete_cleaning_files() {
    let dir = TempDir::new("cleanup");
    let traces = dir.path.join("traces");
    fs::create_dir_all(traces.join("t1")).expect("arborescence du mode");
    fs::create_dir_all(traces.join("t2")).expect("arborescence du mode");

    let doomed = [
        traces.join("t1/cleaning.spike.json"),
        traces.join("t1/cleaning.roundabout.decisions.json"),
        traces.join("t2/cleaning.out_and_back.json"),
        traces.join("t2/cleaning.spike.decisions.json"),
    ];
    let kept = [
        traces.join("t1/trace.gpx"),
        traces.join("t1/trace.gpx.orig"),
        traces.join("t1/trace.geojson"),
        traces.join("t1/keyframes_169.json"),
        traces.join("t1/cleaning.txt"),
        traces.join("t2/cleaning.json.bak"),
        traces.join("cleaning.racine.json"),
    ];
    for p in doomed.iter().chain(kept.iter()) {
        fs::write(p, "x").expect("écriture du fichier de test");
    }

    cleanup_obsolete_cleaning_files(&dir.path);

    for p in doomed.iter() {
        assert!(!p.exists(), "supprimé : {}", p.display());
    }
    for p in kept.iter() {
        assert!(p.exists(), "conservé : {}", p.display());
    }
}

/// D2c / D3b — la migration est idempotente et silencieuse : mode sans dossier
/// de traces, second passage, ou `traces` qui n'est pas un dossier.
#[test]
fn test_cleanup_idempotent() {
    let dir = TempDir::new("cleanup_idem");
    cleanup_obsolete_cleaning_files(&dir.path); // aucun dossier « traces »

    let traces = dir.path.join("traces");
    fs::create_dir_all(traces.join("t1")).expect("arborescence");
    let file = traces.join("t1/cleaning.spike.json");
    fs::write(&file, "x").expect("écriture");

    cleanup_obsolete_cleaning_files(&dir.path);
    assert!(!file.exists());
    cleanup_obsolete_cleaning_files(&dir.path); // second passage sans effet
    assert_eq!(
        fs::read_dir(traces.join("t1")).expect("lecture").count(),
        0
    );

    let dir2 = TempDir::new("cleanup_file");
    fs::write(dir2.path.join("traces"), "pas un dossier").expect("écriture");
    cleanup_obsolete_cleaning_files(&dir2.path);
    assert!(dir2.path.join("traces").exists(), "entrée non dossier ignorée");
}

/// §20.3 — structure du document produit : déclaration, balise `<gpx>`, metadata
/// minimale (repli), trace et point de fermeture.
#[test]
fn test_rewrite_gpx_basic() {
    let dir = TempDir::new("basic");
    let path = write_source_gpx(&dir.path, "trace.gpx", SOURCE_GPX);

    rewrite_gpx(
        &path,
        &make_points(3),
        None,
        None,
        None,
        "VérificationGPX",
        FindingsSummary::default(),
    )
    .expect("réécriture");

    let xml = fs::read_to_string(&path).expect("lecture du GPX");
    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
    assert!(xml.ends_with("</gpx>\n"));
    assert!(
        xml.contains(
            "<gpx version=\"1.1\" creator=\"VérificationGPX\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n"
        ),
        "attributs de repli (aucun attribut source)"
    );
    assert!(xml.contains("  <metadata>\n    <name>trace</name>\n"), "repli : nom de fichier");
    assert!(xml.contains("  <trk><name>trace</name><trkseg>\n"));
    assert_eq!(xml.matches("<trkpt ").count(), 3);
    assert!(xml.contains("  </trkseg></trk>\n"));
}

/// §20.3 — le backup `{nom}.gpx.orig` est créé et contient le GPX d'origine.
#[test]
fn test_rewrite_gpx_backup_created() {
    let dir = TempDir::new("backup");
    let path = write_source_gpx(&dir.path, "trace.gpx", SOURCE_GPX);

    rewrite_gpx(
        &path,
        &make_points(3),
        None,
        None,
        None,
        "VérificationGPX",
        FindingsSummary::default(),
    )
    .expect("réécriture");

    let orig = dir.path.join("trace.gpx.orig");
    assert!(orig.exists(), "backup créé au premier passage");
    assert_eq!(
        fs::read_to_string(&orig).expect("lecture du backup"),
        SOURCE_GPX,
        "le backup est la copie conforme du source"
    );
    assert_ne!(
        fs::read_to_string(&path).expect("lecture"),
        SOURCE_GPX,
        "le GPX a bien été réécrit"
    );
}

/// §20.3 — un backup déjà présent n'est **jamais** écrasé.
#[test]
fn test_rewrite_gpx_backup_preserved() {
    let dir = TempDir::new("backup_keep");
    let path = write_source_gpx(&dir.path, "trace.gpx", SOURCE_GPX);
    let orig = dir.path.join("trace.gpx.orig");
    fs::write(&orig, "SENTINEL").expect("backup sentinelle");

    rewrite_gpx(
        &path,
        &make_points(3),
        None,
        None,
        None,
        "VérificationGPX",
        FindingsSummary::default(),
    )
    .expect("réécriture");

    assert_eq!(
        fs::read_to_string(&orig).expect("lecture du backup"),
        "SENTINEL",
        "le backup d'origine est conservé"
    );
}

/// §20.1 — l'entête du source est réémise : attributs de `<gpx>` à l'identique,
/// `<name>` de trace (casse et espaces conservés), enfants du `<metadata>`
/// préservés, `<desc>` remplacé, bloc d'audit en dernière position.
#[test]
fn test_rewrite_gpx_preserves_attrs() {
    let dir = TempDir::new("attrs");
    let path = write_source_gpx(&dir.path, "trace.gpx", SOURCE_GPX);

    rewrite_gpx(
        &path,
        &make_points(2),
        Some(SOURCE_META),
        Some(SOURCE_ATTRS),
        Some("Ma Trace 2026"),
        "VérificationGPX",
        FindingsSummary::default(),
    )
    .expect("réécriture");

    let xml = fs::read_to_string(&path).expect("lecture");
    assert!(xml.contains(&format!("<gpx {}>\n", SOURCE_ATTRS)), "attributs verbatim");
    assert!(xml.contains("  <trk><name>Ma Trace 2026</name><trkseg>\n"));
    assert!(xml.contains("<name>Sortie du dimanche</name>"));
    assert!(xml.contains("<author><name>JM</name></author>"));
    assert!(xml.contains("<time>2026-01-01T08:00:00Z</time>"));
    assert!(!xml.contains("Trace d'origine"), "ancien <desc> retiré");
    assert_eq!(
        xml.matches("<desc>Trace auditée et corrigée avec VérificationGPX</desc>")
            .count(),
        1,
        "nouveau <desc> posé une seule fois"
    );
    let audit = xml.find("<extensions>").expect("bloc d'audit");
    let close = xml.find("</metadata>").expect("fermeture metadata");
    assert!(audit < close, "bloc d'audit dans le <metadata>");
    assert!(xml.contains("    </audit>\n  </extensions>\n</metadata>"), "en dernier enfant");
}

/// §20.3 — `<ele>` n'est écrit que pour une élévation finie et vaut 2 décimales.
#[test]
fn test_rewrite_gpx_ele_optional() {
    let dir = TempDir::new("ele");
    let path = write_source_gpx(&dir.path, "trace.gpx", SOURCE_GPX);
    let points = vec![
        AuditPoint {
            id: 1,
            lat: 45.0,
            lon: 3.0,
            ele: Some(123.456),
        },
        AuditPoint {
            id: 2,
            lat: 45.1,
            lon: 3.0,
            ele: None,
        },
        AuditPoint {
            id: 3,
            lat: 45.2,
            lon: 3.0,
            ele: Some(f64::NAN),
        },
    ];

    rewrite_gpx(
        &path,
        &points,
        None,
        None,
        None,
        "VérificationGPX",
        FindingsSummary::default(),
    )
    .expect("réécriture");

    let xml = fs::read_to_string(&path).expect("lecture");
    assert!(xml.contains("    <trkpt lat=\"45.0000000\" lon=\"3.0000000\"><ele>123.46</ele></trkpt>\n"));
    assert!(xml.contains("    <trkpt lat=\"45.1000000\" lon=\"3.0000000\"></trkpt>\n"));
    assert!(
        xml.contains("    <trkpt lat=\"45.2000000\" lon=\"3.0000000\"></trkpt>\n"),
        "élévation non finie ignorée"
    );
    assert_eq!(xml.matches("<ele>").count(), 1);
}

/// §20.3 — coordonnées en **7 décimales fixes**, attributs dans l'ordre
/// `lat` puis `lon`.
#[test]
fn test_rewrite_gpx_7_decimals() {
    let dir = TempDir::new("decimals");
    let path = write_source_gpx(&dir.path, "trace.gpx", SOURCE_GPX);
    let points = vec![
        AuditPoint {
            id: 1,
            lat: 45.1,
            lon: 3.0,
            ele: None,
        },
        AuditPoint {
            id: 2,
            lat: 45.1234567,
            lon: -1.9876543,
            ele: None,
        },
    ];

    rewrite_gpx(
        &path,
        &points,
        None,
        None,
        None,
        "VérificationGPX",
        FindingsSummary::default(),
    )
    .expect("réécriture");

    let xml = fs::read_to_string(&path).expect("lecture");
    assert!(
        xml.contains("<trkpt lat=\"45.1000000\" lon=\"3.0000000\">"),
        "7 décimales fixes"
    );
    assert!(
        xml.contains("<trkpt lat=\"45.1234567\" lon=\"-1.9876543\">"),
        "ordre lat/lon et précision conservée"
    );
}

/// §20.3 — écriture atomique : aucun artefact temporaire ne subsiste, le fichier
/// est complet, et un refus (source absente) ne crée ni `.tmp` ni `.orig`.
#[test]
fn test_rewrite_gpx_atomic_write() {
    let dir = TempDir::new("atomic");
    let path = write_source_gpx(&dir.path, "trace.gpx", SOURCE_GPX);

    rewrite_gpx(
        &path,
        &make_points(4),
        None,
        None,
        None,
        "VérificationGPX",
        FindingsSummary::default(),
    )
    .expect("réécriture");

    let mut names: Vec<String> = fs::read_dir(&dir.path)
        .expect("lecture du dossier")
        .map(|e| e.expect("entrée").file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec!["trace.gpx".to_string(), "trace.gpx.orig".to_string()],
        "aucun fichier temporaire résiduel"
    );
    let xml = fs::read_to_string(&path).expect("lecture");
    assert!(xml.ends_with("</gpx>\n"));
    assert_eq!(xml.matches("<trkpt ").count(), 4, "fichier complet");

    let missing = dir.path.join("absent.gpx");
    let err = rewrite_gpx(
        &missing,
        &make_points(2),
        None,
        None,
        None,
        "VérificationGPX",
        FindingsSummary::default(),
    )
    .expect_err("source absente refusée");
    assert!(err.contains("introuvable"), "message attendu : {err}");
    assert!(!dir.path.join("absent.gpx.orig").exists());
    assert!(!dir.path.join("absent.gpx.tmp").exists());
}

/// D12 / §20.2 — le bloc d'audit est déterministe : compteurs dérivés de l'état
/// des findings (`total` = somme), horodatage ISO 8601 UTC à la seconde, et
/// repli du nom d'application quand il est vide.
#[test]
fn test_rewrite_gpx_audit_block() {
    let dir = TempDir::new("audit");
    let summary = FindingsSummary {
        routes: 2,
        deletions: 3,
        false_positives: 1,
    };

    let path = write_source_gpx(&dir.path, "trace.gpx", SOURCE_GPX);
    rewrite_gpx(
        &path,
        &make_points(3),
        Some(SOURCE_META),
        Some(SOURCE_ATTRS),
        Some("Ma Trace 2026"),
        "VérificationGPX",
        summary,
    )
    .expect("réécriture");
    let xml = fs::read_to_string(&path).expect("lecture");

    assert!(xml.contains(
        "  <extensions>\n    <audit xmlns=\"http://VérificationGPX.example/gpx/audit/1\">\n      <modified>"
    ));
    assert!(xml.contains("\n      <tool>VérificationGPX</tool>\n"));
    assert!(
        xml.contains("<corrections total=\"6\" routes=\"2\" deletions=\"3\" falsePositives=\"1\"/>\n"),
        "total = routes + deletions + falsePositives"
    );

    let modified = xml
        .split("<modified>")
        .nth(1)
        .expect("horodatage")
        .split("</modified>")
        .next()
        .expect("fermeture de l'horodatage");
    assert_eq!(modified.len(), 20, "AAAA-MM-JJThh:mm:ssZ attendu : {modified}");
    assert!(modified.ends_with('Z'));

    // Déterminisme : une seconde écriture produit les mêmes compteurs.
    let path2 = write_source_gpx(&dir.path, "trace2.gpx", SOURCE_GPX);
    rewrite_gpx(
        &path2,
        &make_points(3),
        Some(SOURCE_META),
        Some(SOURCE_ATTRS),
        Some("Ma Trace 2026"),
        "VérificationGPX",
        summary,
    )
    .expect("seconde réécriture");
    let xml2 = fs::read_to_string(&path2).expect("lecture");
    let counters = |s: &str| {
        s.split("<corrections ")
            .nth(1)
            .expect("compteurs")
            .split("/>")
            .next()
            .expect("fin des compteurs")
            .to_string()
    };
    assert_eq!(counters(&xml), counters(&xml2));

    // Nom d'application vide ou blanc → repli sur le défaut (IHM §20.4).
    let path3 = write_source_gpx(&dir.path, "trace3.gpx", SOURCE_GPX);
    rewrite_gpx(
        &path3,
        &make_points(2),
        None,
        None,
        None,
        "   ",
        FindingsSummary::default(),
    )
    .expect("réécriture");
    assert!(fs::read_to_string(&path3)
        .expect("lecture")
        .contains("<tool>VérificationGPX</tool>"));
}

// ─── Scénario D11 — double tour, correction réduite et re-détection ──

/// Relit les points d'un GPX exporté (mêmes conventions que le parseur d'import).
fn read_gpx_points(path: &Path) -> Vec<AuditPoint> {
    let file = File::open(path).expect("GPX exporté illisible");
    let gpx = gpx::read(BufReader::new(file)).expect("GPX exporté invalide");
    let mut points = Vec::new();
    for track in &gpx.tracks {
        for segment in &track.segments {
            for wp in &segment.points {
                let p = wp.point();
                points.push(AuditPoint {
                    id: points.len() as u32,
                    lat: p.y(),
                    lon: p.x(),
                    ele: None,
                });
            }
        }
    }
    points
}

/// Détection RP sur une trace déjà en mémoire.
fn detect_rp_on(points: &[AuditPoint], params: &AuditParams) -> Vec<Finding> {
    let geo = build_geometry(points);
    detect_rp(points, &geo.px, &geo.py, &geo.cum, &geo.ids, geo.total, params)
}

/// D11 / C10 — double tour : la trace de référence porte un giratoire parcouru
/// plus d'une fois (776° — « 2 tours complets + 1/4 de tour »). Une suppression
/// RP **réduite** retire le tour redondant ; le GPX exporté, relu et re-détecté,
/// ne présente plus de boucle.
#[test]
fn test_scenario_d11_double_tour_reduced_then_redetect() {
    let params = super::rp_test::rp_params();
    let raw = super::rp_test::load_gpx("RP_erreur_Magny");
    let (kept, _removed) = consolidate_points(&raw, params.consol_m);
    let findings = detect_rp_on(&kept, &params);
    assert_eq!(findings.len(), 1, "un giratoire détecté");
    assert_eq!(findings[0].total_angle, Some(776));
    assert_eq!(
        findings[0].turn_text.as_deref(),
        Some("2 tours complets + 1/4 de tour")
    );
    let (cs, ce) = (findings[0].parts[1].s, findings[0].parts[1].e); // cœur 39..61

    // Réduction : le tour redondant du cœur est retiré.
    let state = apply_delete(kept, findings, "rp-1", cs + 2, ce - 2, 1000)
        .expect("suppression RP réduite");
    assert!(
        detect_rp_on(&state.points, &params).is_empty(),
        "le tour redondant a disparu de la trace de travail"
    );

    // Export du GPX corrigé, relecture et re-détection (C10 : la trace exportée
    // est exactement la trace de travail).
    let dir = TempDir::new("d11");
    let path = write_source_gpx(&dir.path, "trace.gpx", SOURCE_GPX);
    let summary = FindingsSummary {
        routes: 0,
        deletions: 1,
        false_positives: 0,
    };
    rewrite_gpx(&path, &state.points, None, None, None, "VérificationGPX", summary)
        .expect("export du GPX corrigé");

    let exported = fs::read_to_string(&path).expect("lecture du GPX exporté");
    assert!(
        exported.contains("<corrections total=\"1\" routes=\"0\" deletions=\"1\" falsePositives=\"0\"/>"),
        "les compteurs d'audit accompagnent le GPX corrigé"
    );

    let reloaded = read_gpx_points(&path);
    assert_eq!(reloaded.len(), state.points.len(), "trace exportée complète");
    let (reloaded_kept, _) = consolidate_points(&reloaded, params.consol_m);
    assert!(
        detect_rp_on(&reloaded_kept, &params).is_empty(),
        "plus aucune boucle détectée sur le GPX exporté"
    );

    // Sensibilité du verdict : une réduction plus faible laisse le tour
    // redondant, donc une boucle toujours détectée (angle réduit).
    let partial = apply_delete(
        super::rp_test::load_gpx("RP_erreur_Magny"),
        detect_rp_on(&consolidate_points(&super::rp_test::load_gpx("RP_erreur_Magny"), params.consol_m).0, &params),
        "rp-1",
        cs + 6,
        ce - 6,
        1000,
    );
    if let Ok(st) = partial {
        let still = detect_rp_on(&st.points, &params);
        assert_eq!(still.len(), 1, "boucle encore détectée après réduction partielle");
        assert!(
            still[0].total_angle.unwrap_or(0).abs() < 776,
            "angle cumulé réduit"
        );
    }
}
