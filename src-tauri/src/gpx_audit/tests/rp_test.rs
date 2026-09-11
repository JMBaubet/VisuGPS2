//! Tests du détecteur RP.
//!
//! Sous-étape 2.1 : `resample_geo` (rééchantillonnage), `rp_norm_180`
//! (normalisation angulaire) et `turn_text` (traduction de la rotation).
//! Sous-étape 2.2 : `compute_heading_prefix` / `angle_between` (caps lissés et
//! cumul angulaire), `find_proximity_candidates` (É1), `segs_cross` et
//! `find_crossing_candidates` (É2).
//! Sous-étape 2.3 : `merge_candidates` (É3) et `refine_pairs` /
//! `min_pair_distance` (É4a).
//! Sous-étape 2.4 : `rp_walk`, `closing_turn`, `closing_delta`,
//! `loop_degenerate` (anti-aiguille), `push_window` (É4b/c) et
//! `fill_pairs_in` / `select_windows` (É4d/e).
//! Sous-étape 2.5 : `publish_windows` et `detect_rp` (É5) ; `anchor.rs`
//! (`fit_circle_kasa`, `rp_anchor_indices`) ; scénarios §17.2.

use crate::gpx_audit::anchor::{fit_circle_kasa, rp_anchor_indices};
use crate::gpx_audit::consolidation::consolidate_points;
use crate::gpx_audit::geometry::build_geometry;
use crate::gpx_audit::rp::{
    angle_between, closing_delta, closing_turn, compute_heading_prefix, detect_rp, fill_pairs_in,
    find_crossing_candidates, find_proximity_candidates, loop_degenerate, merge_candidates,
    min_pair_distance, publish_windows, push_window, refine_pairs, resample_geo, rp_norm_180,
    rp_walk, segs_cross, select_windows, turn_text, Candidate, MergedGroup, RefinedPair,
    ResampledGeo, Window, WindowCtx, RP_MAX_PAIRS, RP_MAX_SAMPLES, RP_MIN_SEP, RP_PAIRS_TOP,
    RP_STEP_DEFAULT, RP_WALK_MAX_SEG, RP_WALK_MIN_M,
};
use crate::gpx_audit::types::{AuditParams, AuditPoint, Finding, FindingKind, FindingStatus};
use std::f64::consts::PI;
use std::fs::File;
use std::io::BufReader;

/// Compare une valeur flottante à une valeur attendue (tolérance 1e-9).
fn assert_close(actual: f64, expected: f64) {
    assert_close_tol(actual, expected, 1e-9);
}

/// Compare une valeur flottante à une valeur attendue avec une tolérance donnée.
fn assert_close_tol(actual: f64, expected: f64, tol: f64) {
    assert!(
        (actual - expected).abs() < tol,
        "valeur attendue ~{expected} (±{tol}), obtenue {actual}"
    );
}

/// Construit une structure `R` de test à partir d'échantillons explicites :
/// `n = len − 1`, `cumR` cumule les cordes, `orig` est l'identité.
fn r_from(samples: &[(f64, f64)], step: f64) -> ResampledGeo {
    let n = samples.len() - 1;
    let mut rx = Vec::with_capacity(n + 1);
    let mut ry = Vec::with_capacity(n + 1);
    let mut cum_r = vec![0.0; n + 1];
    for (i, &(x, y)) in samples.iter().enumerate() {
        rx.push(x);
        ry.push(y);
        if i > 0 {
            cum_r[i] = cum_r[i - 1] + (x - rx[i - 1]).hypot(y - ry[i - 1]);
        }
    }
    ResampledGeo {
        n,
        rx,
        ry,
        cum_r,
        orig: (0..=n).collect(),
        step,
    }
}

/// Candidat de test produit par l'É1 (paire par proximité).
fn cand_pair(s: usize, e: usize) -> Candidate {
    Candidate {
        s,
        e,
        paire: Some((s, e)),
        croisement: None,
    }
}

/// Candidat de test produit par l'É2 (croisement de segments).
fn cand_cross(s: usize, e: usize) -> Candidate {
    Candidate {
        s,
        e,
        paire: None,
        croisement: Some((s, e)),
    }
}

/// Groupe fusionné de test portant les paires raffinables données.
fn group(paires: Vec<(usize, usize)>) -> MergedGroup {
    MergedGroup {
        s: 0,
        e: 0,
        paires,
        croisements: Vec::new(),
    }
}

/// Points de référence d'un aller-retour exact : sortie vers l'est de 10 m en
/// 10 m (P0 → P3) puis retour sur les pas (P3 → P6) — P0 et P6 sont superposés.
fn out_and_back_ref() -> (Vec<f64>, Vec<f64>) {
    (vec![0.0, 10.0, 20.0, 30.0, 20.0, 10.0, 0.0], vec![0.0; 7])
}

/// Trace rééchantillonnée de test : 31 échantillons espacés de 1 m le long de
/// l'aller-retour, chaque échantillon hébergeant le point de référence le plus
/// proche (`orig[i] = round(i/5)`, soit 5 échantillons par point).
fn r_out_and_back() -> ResampledGeo {
    let samples: Vec<(f64, f64)> = (0..=30).map(|i| (i as f64, 0.0)).collect();
    let mut r = r_from(&samples, 4.0);
    r.orig = (0..=30)
        .map(|i| ((i as f64) / 5.0).round() as usize)
        .collect();
    r
}

/// Échantillons d'un cercle de rayon `r` en `n + 1` points d'angle, parcouru
/// dans le sens trigonométrique (cumul angulaire positif). Le tour est fermé :
/// le dernier échantillon est confondu avec le premier.
fn circle(r: f64, n: usize) -> ResampledGeo {
    let pts: Vec<(f64, f64)> = (0..=n)
        .map(|i| {
            let t = i as f64 * 2.0 * PI / n as f64;
            (r * t.cos(), r * t.sin())
        })
        .collect();
    r_from(&pts, 4.0)
}

/// Fenêtre de test (É4b).
fn window(a: usize, b: usize, d: Option<f64>, total: f64, propre: bool) -> Window {
    Window {
        a,
        b,
        d,
        total,
        propre,
        pairs_in: Vec::new(),
    }
}

/// Giratoire de rayon 20 m à voie d'entrée/sortie commune : approche vers
/// l'est, un tour complet dans le sens trigonométrique, puis sortie dans la
/// direction `exit_deg` (0 = est, 90 = nord) — le point de sortie est confondu
/// avec le point d'entrée. Retourne `R`, l'index de jonction (début de boucle)
/// et l'index de refermeture (fin de boucle).
fn roundabout(exit_deg: f64) -> (ResampledGeo, usize, usize) {
    let mut pts: Vec<(f64, f64)> = (0..=5)
        .map(|i| (-60.0 + 12.0 * i as f64, 0.0))
        .collect();
    let a = pts.len() - 1; // jonction : (0, 0)
    for i in 1..=40 {
        let t = i as f64 * 2.0 * PI / 40.0;
        pts.push((20.0 * t.sin(), 20.0 - 20.0 * t.cos()));
    }
    let b = pts.len() - 1; // refermeture : retour en (0, 0)
    let rad = exit_deg * PI / 180.0;
    for i in 1..=3 {
        pts.push((20.0 * i as f64 * rad.cos(), 20.0 * i as f64 * rad.sin()));
    }
    (r_from(&pts, 4.0), a, b)
}

/// Boucle seule : giratoire de rayon `r` centré en `(0, r)`, refermé sur son
/// point de départ `(0, 0)` — `n + 1` points d'angle.
fn loop_only(r: f64, n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut px = Vec::new();
    let mut py = Vec::new();
    for i in 0..=n {
        let t = i as f64 * 2.0 * PI / n as f64;
        px.push(r * t.sin());
        py.push(r - r * t.cos());
    }
    (px, py)
}

/// Géométrie de référence des ancres : approche rectiligne de 200 m (1 point
/// par 10 m), giratoire de rayon 20 m refermé en `(0, 0)`, sortie rectiligne de
/// 200 m. Retourne `(px, py, junc, ce)`.
fn roundabout_axes() -> (Vec<f64>, Vec<f64>, usize, usize) {
    let (mut px, mut py) = approach();
    let junc = px.len() - 1; // 20 → (0, 0)
    push_loop(&mut px, &mut py);
    let ce = px.len() - 1; // 60 → (0, 0)
    for i in 1..=20 {
        px.push(10.0 * i as f64);
        py.push(0.0);
    }
    (px, py, junc, ce)
}

/// Même géométrie, mais la sortie contient un échantillon aberrant isolé
/// (index 62 : 60 m hors de la route) entre deux points en zone.
fn roundabout_axes_with_outlier() -> (Vec<f64>, Vec<f64>, usize, usize) {
    let (mut px, mut py) = approach();
    let junc = px.len() - 1;
    push_loop(&mut px, &mut py);
    let ce = px.len() - 1;
    px.push(10.0);
    py.push(0.0); // 61
    px.push(15.0);
    py.push(-60.0); // 62 : aberrant hors zone, isolé
    px.push(20.0);
    py.push(0.0); // 63
    for i in 3..=6 {
        px.push(10.0 * i as f64);
        py.push(0.0); // 64 (30 m) … 67 (60 m)
    }
    (px, py, junc, ce)
}

/// Boucle en tête de trace, précédée d'un unique point hors zone : la sortie
/// de zone coïncide avec la borne de trace (cas C1 des ancres).
fn loop_with_lead_out_of_zone() -> (Vec<f64>, Vec<f64>) {
    let mut px = vec![40.0]; // index 0 : hors zone, en bord de trace
    let mut py = vec![0.0];
    let (lx, ly) = loop_only(20.0, 40);
    px.extend(lx);
    py.extend(ly);
    (px, py)
}

/// Approche rectiligne de 200 m vers l'est, 1 point par 10 m (indices 0 à 20,
/// le dernier valant `(0, 0)`).
fn approach() -> (Vec<f64>, Vec<f64>) {
    (
        (0..=20).map(|i| -200.0 + 10.0 * i as f64).collect(),
        vec![0.0; 21],
    )
}

/// Ajoute un demi-tour de giratoire de rayon 20 m (40 points d'angle, le
/// dernier revenant en `(0, 0)`).
fn push_loop(px: &mut Vec<f64>, py: &mut Vec<f64>) {
    for i in 1..=40 {
        let t = i as f64 * 2.0 * PI / 40.0;
        px.push(20.0 * t.sin());
        py.push(20.0 - 20.0 * t.cos());
    }
}

/// `R` de test pour l'É5 : 101 échantillons hébergeant les 60 points de
/// référence d'une ligne droite (`orig[i] = i/2`, le dernier échantillon
/// portant le dernier point), avec des `ids` décalés pour prouver leur usage.
fn e5_fixture() -> (ResampledGeo, Vec<f64>, Vec<f64>, Vec<u32>) {
    let m = 60usize;
    let px: Vec<f64> = (0..m).map(|i| 10.0 * i as f64).collect();
    let py: Vec<f64> = vec![0.0; m];
    let ids: Vec<u32> = (0..m).map(|i| 1000 + i as u32).collect();

    let n = 100usize;
    let ref_of = |i: usize| if i == n { m - 1 } else { i / 2 };
    let samples: Vec<(f64, f64)> = (0..=n).map(|i| (px[ref_of(i)], 0.0)).collect();
    let mut r = r_from(&samples, 4.0);
    r.orig = (0..=n).map(ref_of).collect();
    (r, px, py, ids)
}

/// Paramètres RP par défaut (ANALYSE §3.1) : σ = 0,5 m, p-close = 15 m,
/// p-angle = 270°.
fn rp_params() -> AuditParams {
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

/// Charge un fichier GPX de test en résolvant son nom par **préfixe ASCII** :
/// selon le système de fichiers, les noms peuvent être stockés en NFC ou en NFD
/// (macOS utilise NFD pour « AR_Detecté aussi en RP.gpx ») et Rust ne normalise
/// pas Unicode sans dépendance externe.
fn load_gpx(prefix: &str) -> Vec<AuditPoint> {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../docs/audit/reference/test_files"
    );
    let path = std::fs::read_dir(dir)
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
        .unwrap_or_else(|| panic!("fichier de test introuvable : {prefix}*.gpx"));

    let file = File::open(&path).expect("fichier GPX illisible");
    let gpx = gpx::read(BufReader::new(file)).expect("GPX invalide");
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

/// Consolidation + géométrie + détection RP sur un fichier de test.
fn detect_scenario(prefix: &str) -> Vec<Finding> {
    let raw = load_gpx(prefix);
    let params = rp_params();
    let (kept, _removed) = consolidate_points(&raw, params.consol_m);
    let geo = build_geometry(&kept);
    detect_rp(
        &kept,
        &geo.px,
        &geo.py,
        &geo.cum,
        &geo.ids,
        geo.total,
        &params,
    )
}

// ─── Sous-étape 2.1 : resample_geo ───────────────────────────────────

#[test]
fn test_resample_geo_linear() {
    // Ligne droite de 40 m échantillonnée à 4 m : 10 intervalles réguliers.
    // Le segment hôte n'avance qu'à la fin du segment courant (cum[k+1] >= d).
    let px = [0.0, 10.0, 20.0, 30.0, 40.0];
    let py = [0.0; 5];
    let cum = [0.0, 10.0, 20.0, 30.0, 40.0];
    let r = resample_geo(&px, &py, &cum, 40.0, 4.0);

    assert_eq!(r.n, 10);
    assert_close(r.step, 4.0);
    assert_eq!(r.rx.len(), 11);
    for i in 0..=10 {
        let d = 4.0 * i as f64;
        assert_close(r.rx[i], d);
        assert_close(r.ry[i], 0.0);
        assert_close(r.cum_r[i], d);
    }
    // orig : l'échantillon final est rattaché à m−1, les autres héritent du
    // segment hôte (cum[k+1] < d fait avancer k).
    assert_eq!(r.orig, vec![0, 0, 0, 1, 1, 1, 2, 2, 3, 3, 4]);
}

#[test]
fn test_resample_geo_loop() {
    // Boucle carrée de 40 m de périmètre (10 m par côté) : les échantillons
    // suivent les quatre côtés et le dernier retombe exactement sur P₀.
    let px = [0.0, 0.0, 10.0, 10.0, 0.0];
    let py = [0.0, 10.0, 10.0, 0.0, 0.0];
    let cum = [0.0, 10.0, 20.0, 30.0, 40.0];
    let r = resample_geo(&px, &py, &cum, 40.0, 4.0);

    assert_eq!(r.n, 10);
    assert_close(r.step, 4.0);
    assert_eq!(r.rx.len(), 11);
    // Interpolation linéaire le long des côtés de la boucle.
    assert_close(r.rx[0], 0.0);
    assert_close(r.ry[0], 0.0);
    assert_close(r.rx[1], 0.0);
    assert_close(r.ry[1], 4.0);
    assert_close(r.rx[2], 0.0);
    assert_close(r.ry[2], 8.0);
    assert_close(r.rx[3], 2.0);
    assert_close(r.ry[3], 10.0);
    assert_close(r.rx[5], 10.0);
    assert_close(r.ry[5], 10.0);
    // Refermeture : le dernier échantillon revient sur le point de départ.
    assert_close(r.rx[10], 0.0);
    assert_close(r.ry[10], 0.0);
    // cumR est la somme des CORDES de la polyligne rééchantillonnée, pas la
    // longueur d'arc source : aux angles saillants, la corde qui relie les deux
    // échantillons encadrant le coin le coupe. Les coins d'arc 10 et d'arc 30
    // sont ainsi coupés par une corde de 2√2 m au lieu de 4 m d'arc, d'où
    // cumR[10] = 40 − 2·(4 − 2√2) = 32 + 4√2.
    assert_close(r.cum_r[1], 4.0);
    assert_close(r.cum_r[2], 8.0);
    assert_close(r.cum_r[3], 8.0 + 2.0 * 2.0_f64.sqrt());
    assert_close(r.cum_r[10], 32.0 + 4.0 * 2.0_f64.sqrt());
    for i in 1..=10 {
        assert!(r.cum_r[i] > r.cum_r[i - 1], "cumR doit croître en i = {i}");
    }
    // orig est monotone croissant et borné par m−1.
    assert_eq!(r.orig[0], 0);
    assert_eq!(r.orig[10], 4);
    for i in 1..=10 {
        assert!(r.orig[i] >= r.orig[i - 1], "orig non monotone en i = {i}");
        assert!(r.orig[i] <= 4);
    }
}

#[test]
fn test_resample_geo_max_samples() {
    // 1 000 km à 4 m demanderaient 250 000 échantillons : plafonnés à
    // RP_MAX_SAMPLES, le pas effectif est recalculé en conséquence.
    let px = [0.0, 1_000_000.0];
    let py = [0.0, 0.0];
    let cum = [0.0, 1_000_000.0];
    let r = resample_geo(&px, &py, &cum, 1_000_000.0, 4.0);

    assert_eq!(r.n, RP_MAX_SAMPLES);
    assert_eq!(r.rx.len(), RP_MAX_SAMPLES + 1);
    assert_close(r.step, 1_000_000.0 / RP_MAX_SAMPLES as f64);
    assert_eq!(r.orig[r.n], 1);
    assert_close(r.cum_r[r.n], 1_000_000.0);
}

#[test]
fn test_resample_geo_zero_length() {
    // Trace immobile (total = 0) : n = max(1, round(0)) = 1, pas effectif 0,
    // les deux échantillons restent sur P₀ et cumR reste nul.
    let px = [5.0, 5.0, 5.0];
    let py = [7.0, 7.0, 7.0];
    let cum = [0.0, 0.0, 0.0];
    let r = resample_geo(&px, &py, &cum, 0.0, 4.0);

    assert_eq!(r.n, 1);
    assert_close(r.step, 0.0);
    assert_eq!(r.rx.len(), 2);
    assert_close(r.rx[0], 5.0);
    assert_close(r.rx[1], 5.0);
    assert_close(r.ry[0], 7.0);
    assert_close(r.ry[1], 7.0);
    assert_close(r.cum_r[0], 0.0);
    assert_close(r.cum_r[1], 0.0);
    assert_eq!(r.orig, vec![0, 2]);
}

// ─── Sous-étape 2.1 : rp_norm_180 ────────────────────────────────────

#[test]
fn test_rp_norm_180_positive() {
    // Les angles positifs restent inchangés jusqu'à 180, puis basculent
    // dans le demi-plan négatif (350° → −10°).
    assert_close(rp_norm_180(0.0), 0.0);
    assert_close(rp_norm_180(10.0), 10.0);
    assert_close(rp_norm_180(90.0), 90.0);
    assert_close(rp_norm_180(170.0), 170.0);
    assert_close(rp_norm_180(190.0), -170.0);
    assert_close(rp_norm_180(270.0), -90.0);
    assert_close(rp_norm_180(350.0), -10.0);
    assert_close(rp_norm_180(359.0), -1.0);
}

#[test]
fn test_rp_norm_180_negative() {
    // Symétrie : les angles négatifs remontent au-delà de −180 (−190 → 170).
    assert_close(rp_norm_180(-10.0), -10.0);
    assert_close(rp_norm_180(-90.0), -90.0);
    assert_close(rp_norm_180(-170.0), -170.0);
    assert_close(rp_norm_180(-190.0), 170.0);
    assert_close(rp_norm_180(-270.0), 90.0);
    assert_close(rp_norm_180(-350.0), 10.0);
}

#[test]
fn test_rp_norm_180_boundary() {
    // Bornes : ±180 et leurs multiples tombent sur +180 (borne supérieure
    // inclusive) ; 720 revient à 0.
    assert_close(rp_norm_180(180.0), 180.0);
    assert_close(rp_norm_180(-180.0), 180.0);
    assert_close(rp_norm_180(540.0), 180.0);
    assert_close(rp_norm_180(-540.0), 180.0);
    assert_close(rp_norm_180(720.0), 0.0);
}

// ─── Sous-étape 2.1 : turn_text ──────────────────────────────────────

#[test]
fn test_turn_text_complete_turn() {
    // Fenêtre de ±30° autour de 360° : « tour complet (1×) ».
    assert_eq!(turn_text(360.0), "tour complet (1×)");
    assert_eq!(turn_text(350.0), "tour complet (1×)");
    assert_eq!(turn_text(330.0), "tour complet (1×)");
    // 320° sort de la fenêtre : le quatrième quart arrondit à « tour complet ».
    assert_eq!(turn_text(320.0), "tour complet");
}

#[test]
fn test_turn_text_multiple_turns() {
    assert_eq!(turn_text(720.0), "2 tours complets");
    assert_eq!(turn_text(1080.0), "3 tours complets");
    assert_eq!(turn_text(1440.0), "4 tours complets");
    assert_eq!(turn_text(725.0), "2 tours complets");
    // Cas littéral du gabarit JS `q === 0` : « base tours complets », pluriel
    // figé y compris pour base = 1 (reproduction à l'identique).
    assert_eq!(turn_text(391.0), "1 tours complets");
}

#[test]
fn test_turn_text_partial() {
    // Trois-quarts de tour (base nulle : le libellé du quart est retourné seul).
    assert_eq!(turn_text(270.0), "3/4 de tour");
    assert_eq!(turn_text(300.0), "3/4 de tour");
    // Base non nulle + quart.
    assert_eq!(turn_text(450.0), "1 tour complet + 1/4 de tour");
    assert_eq!(turn_text(800.0), "2 tours complets + 1/4 de tour");
}

#[test]
fn test_turn_text_uturn() {
    // Sous 240°, la qualification reste « demi-tour dépassé » (plage basse
    // volontairement sans classe de sévérité).
    assert_eq!(turn_text(90.0), "demi-tour dépassé");
    assert_eq!(turn_text(180.0), "demi-tour dépassé");
    assert_eq!(turn_text(200.0), "demi-tour dépassé");
    assert_eq!(turn_text(239.0), "demi-tour dépassé");
    // 240° entre dans la classe des trois-quarts de tour.
    assert_eq!(turn_text(240.0), "3/4 de tour");
}

// ─── Sous-étape 2.2 : cumul angulaire ────────────────────────────────

#[test]
fn test_heading_cumul_straight() {
    // Ligne droite : cap constant, donc cumul rigoureusement nul.
    let pts: Vec<(f64, f64)> = (0..=20)
        .map(|i| (i as f64 * 5.0, i as f64 * 5.0 * 0.577))
        .collect();
    let r = r_from(&pts, 4.0);
    let pre = compute_heading_prefix(&r);
    assert_eq!(pre.len(), r.n + 1);
    for v in &pre {
        assert_close(*v, 0.0);
    }
}

#[test]
fn test_heading_cumul_circle() {
    // Cercle de 30 m parcouru dans le sens trigonométrique, pas angulaire de
    // 10° : le cumul SIGNÉ vaut +360° au bout d'un tour (le dernier intervalle
    // n'est pas cumulé, borne reprise de `pre[n−1]`).
    let pts: Vec<(f64, f64)> = (0..=38)
        .map(|i| {
            let a = i as f64 * 10.0 * PI / 180.0;
            (30.0 * a.cos(), 30.0 * a.sin())
        })
        .collect();
    let r = r_from(&pts, 4.0);
    let pre = compute_heading_prefix(&r);

    assert_close_tol(angle_between(&pre, 0, r.n), 360.0, 1e-6);
    assert_close_tol(angle_between(&pre, 1, r.n), 360.0, 1e-6);
    // Progression monotone : à l'échantillon 20, 190° de rotation cumulée
    // (20 pas de 10° depuis l'échantillon 1).
    assert_close_tol(angle_between(&pre, 1, 20), 190.0, 1e-6);
    for k in 2..=r.n {
        assert!(
            pre[k] >= pre[k - 1],
            "cumul non croissant sur un cercle trigonométrique (k = {k})"
        );
    }
}

#[test]
fn test_heading_cumul_s_curve() {
    // Droite vers l'est, virage à gauche de 90°, virage à droite de 90°, puis
    // droite : les deux rotations s'annulent. C'est ce cumul signé qui
    // distingue un enchaînement en S (somme nulle) d'une boucle (+360°).
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for i in 0..=20 {
        pts.push((i as f64 * 2.5, 0.0)); // droite est
    }
    for i in 1..=90 {
        // arc de rayon 20 centré en (50, 20) : est → nord, pas de 1°
        let t = i as f64 * (PI / 2.0) / 90.0;
        pts.push((50.0 + 20.0 * t.sin(), 20.0 - 20.0 * t.cos()));
    }
    for i in 1..=90 {
        // arc de rayon 20 centré en (90, 20) : nord → est, pas de 1°
        let s = i as f64 * (PI / 2.0) / 90.0;
        pts.push((90.0 - 20.0 * s.cos(), 20.0 + 20.0 * s.sin()));
    }
    for i in 0..=20 {
        pts.push((90.0 + i as f64 * 2.5, 40.0)); // droite est
    }
    let r = r_from(&pts, 4.0);
    let pre = compute_heading_prefix(&r);

    // Les deux extrémités sont sur des droites de même cap (est) : cumul nul.
    assert_close_tol(pre[r.n], 0.0, 1e-9);
    // Pic intermédiaire ≈ +90° au sortir de l'arc gauche, jamais de rotation
    // négative (le second arc ramène à 0 sans dépasser). Le lissage ±1
    // échantillon sous-estime la tangente de la moitié du pas angulaire, d'où
    // la tolérance de 1° avec un pas d'arc de 1°.
    let max = pre.iter().cloned().fold(f64::MIN, f64::max);
    let min = pre.iter().cloned().fold(f64::MAX, f64::min);
    assert_close_tol(max, 90.0, 1.0);
    assert!(min > -1.0, "cumul négatif inattendu : {min}");
}

// ─── Sous-étape 2.2 : É1 (proximité spatiale) ────────────────────────

#[test]
fn test_e1_proximity_grid() {
    // Deux échantillons à 5 m l'un de l'autre mais séparés de 7 indices : la
    // grille de hachage (cellule 15 m) les retrouve sans parcours O(n²).
    let mut pts: Vec<(f64, f64)> = vec![(0.0, 0.0)];
    for k in 1..=6 {
        pts.push((100.0 * k as f64, 0.0));
    }
    pts.push((5.0, 0.0)); // index 7, à 5 m de l'index 0
    let r = r_from(&pts, 4.0);

    let cands = find_proximity_candidates(&r, 15.0);
    assert_eq!(cands.len(), 1);
    assert_eq!((cands[0].s, cands[0].e), (0, 7));
    assert_eq!(cands[0].paire, Some((0, 7)));
    assert!(cands[0].croisement.is_none());
}

#[test]
fn test_e1_min_separation() {
    // Chaîne de points espacés de 2 m : toutes les paires sont sous `closeThr`,
    // mais celles séparées de moins de RP_MIN_SEP indices sont écartées — les
    // paires quasi contiguës ne polluent pas l'É4.
    let pts: Vec<(f64, f64)> = (0..20).map(|i| (i as f64 * 2.0, 0.0)).collect();
    let r = r_from(&pts, 4.0);

    let cands = find_proximity_candidates(&r, 15.0);
    assert!(!cands.is_empty(), "les paires à 12 m et 14 m doivent ressortir");
    for c in &cands {
        assert!(
            c.e - c.s >= RP_MIN_SEP,
            "paire trop serrée retenue : {}..{}",
            c.s,
            c.e
        );
        let d = (r.rx[c.e] - r.rx[c.s]).hypot(r.ry[c.e] - r.ry[c.s]);
        assert!(d < 15.0, "paire au-delà de closeThr : {d} m");
    }
}

#[test]
fn test_e1_max_gap() {
    // La trace revient exactement sur son point de départ après 1100
    // échantillons : la portée maximale (RP_MAX_GAP_M = 4 km, soit 1008
    // échantillons au pas de 4 m) écarte cette paire quelle que soit la
    // proximité spatiale des deux points.
    let mut pts: Vec<(f64, f64)> = vec![(0.0, 0.0)];
    for k in 1..=1099 {
        pts.push((1000.0 + 20.0 * k as f64, 0.0));
    }
    pts.push((0.0, 0.0)); // index 1100 : retour à l'origine
    let r = r_from(&pts, 4.0);

    let cands = find_proximity_candidates(&r, 15.0);
    assert!(cands.is_empty(), "paire hors portée retenue : {cands:?}");
}

#[test]
fn test_e1_max_pairs_truncation() {
    // Tous les échantillons confondus : les paires brutes dépassent largement
    // RP_MAX_PAIRS — la collecte est tronquée proprement à cette borne.
    let pts: Vec<(f64, f64)> = vec![(0.0, 0.0); 301];
    let r = r_from(&pts, 4.0);

    let cands = find_proximity_candidates(&r, 15.0);
    assert_eq!(cands.len(), RP_MAX_PAIRS);
}

#[test]
fn test_e1_deferred_insertion() {
    // Trois échantillons mutuellement proches (indices 0, 6 et 12) : l'insertion
    // différée produit chaque paire (j, i) avec j < i une seule fois, dans
    // l'ordre de découverte des points aval.
    let mut pts: Vec<(f64, f64)> = vec![(0.0, 0.0)];
    for k in 1..=5 {
        pts.push((50.0 * k as f64, 0.0));
    }
    pts.push((10.0, 0.0)); // index 6
    for k in 1..=5 {
        pts.push((200.0 * k as f64, 0.0));
    }
    pts.push((5.0, 0.0)); // index 12
    let r = r_from(&pts, 4.0);

    let cands = find_proximity_candidates(&r, 15.0);
    let pairs: Vec<(usize, usize)> = cands.iter().map(|c| (c.s, c.e)).collect();
    assert_eq!(pairs, vec![(0, 6), (0, 12), (6, 12)]);
}

// ─── Sous-étape 2.2 : É2 (croisements de segments) ───────────────────

#[test]
fn test_e2_segs_cross_strict() {
    // Segment 0 = (0,0)-(10,0), segment 2 = (5,−5)-(5,5) : orientations
    // opposées de part et d'autre des deux segments.
    let r = r_from(&[(0.0, 0.0), (10.0, 0.0), (5.0, -5.0), (5.0, 5.0)], 4.0);
    assert!(segs_cross(&r, 0, 2));

    // Segments parallèles disjoints : ni croisement strict, ni contact.
    let r2 = r_from(&[(0.0, 0.0), (10.0, 0.0), (0.0, 50.0), (10.0, 50.0)], 4.0);
    assert!(!segs_cross(&r2, 0, 2));

    // Indices hors bornes : rejet, sans accès hors tableau.
    assert!(!segs_cross(&r, 0, 3));
}

#[test]
fn test_e2_segs_cross_colinear() {
    // Segments colinéaires qui se chevauchent : les produits vectoriels sont
    // nuls (pas de croisement strict), c'est le contact quasi exact du niveau 2
    // qui les retient (seuil RP_EPS_CROSS).
    let r = r_from(&[(0.0, 0.0), (10.0, 0.0), (5.0, 0.0), (15.0, 0.0)], 4.0);
    assert!(segs_cross(&r, 0, 2));

    // Décalage perpendiculaire de 10 cm : très au-delà du seuil de colinéarité
    // (1e-7 m²), il n'y a plus contact.
    let r2 = r_from(&[(0.0, 0.0), (10.0, 0.0), (5.0, 0.1), (15.0, 0.1)], 4.0);
    assert!(!segs_cross(&r2, 0, 2));
}

#[test]
fn test_e2_no_adjacent() {
    // Triangle fermé : le dernier segment recoupe le premier à son origine
    // (contact niveau 2 avéré), mais k − m = 2 < 3 — les segments adjacents
    // sont exclus, donc aucun candidat.
    let r = r_from(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 0.0)], 4.0);
    assert!(segs_cross(&r, 0, 2), "le contact existe bien");

    let cands = find_crossing_candidates(&r, 15.0);
    assert!(cands.is_empty(), "candidat adjacent retenu : {cands:?}");
}

#[test]
fn test_e2_dedup_key() {
    // Le segment m = [0,1] (0,0)-(48,0) occupe 4 cellules (cell2 = 15) ; la
    // boîte du segment k = [5,6] (10,−10)-(40,10) en couvre 3. Sans la clé de
    // dédoublonnage `m·10⁶ + k`, la paire serait testée — et poussée — 3 fois.
    let pts: Vec<(f64, f64)> = vec![
        (0.0, 0.0),
        (48.0, 0.0),
        (0.0, 1000.0),
        (0.0, 2000.0),
        (0.0, 3000.0),
        (10.0, -10.0),
        (40.0, 10.0),
    ];
    let r = r_from(&pts, 4.0);

    let cands = find_crossing_candidates(&r, 15.0);
    // La paire (0, 6) — c'est-à-dire m = 0 et k = 5 — doit apparaître une seule
    // fois, malgré les trois cellules communes parcourues.
    let mut n_pair = 0;
    for c in &cands {
        if c.s == 0 && c.e == 6 {
            n_pair += 1;
            assert_eq!(c.croisement, Some((0, 6)));
            assert!(c.paire.is_none());
        }
    }
    assert_eq!(n_pair, 1, "paire dédoublonnée attendue une seule fois");
}

// ─── Sous-étape 2.3 : É3 (fusion des candidats) ──────────────────────

#[test]
fn test_e3_fusion_overlap() {
    // Deux candidats qui se recouvrent fusionnent : l'intervalle s'étend à
    // l'enveloppe des deux et les descripteurs s'accumulent dans le groupe.
    let cands = vec![cand_pair(10, 30), cand_pair(20, 40)];
    let merged = merge_candidates(&cands);
    assert_eq!(merged.len(), 1);
    assert_eq!((merged[0].s, merged[0].e), (10, 40));
    assert_eq!(merged[0].paires, vec![(10, 30), (20, 40)]);
    assert!(merged[0].croisements.is_empty());
}

#[test]
fn test_e3_fusion_disjoint() {
    // Candidats disjoints : trois groupes distincts, triés par (s, e) et
    // mutuellement disjoints — la propriété dont dépend le bornage de l'É5.
    let cands = vec![cand_pair(50, 60), cand_pair(0, 10), cand_pair(70, 80)];
    let merged = merge_candidates(&cands);
    assert_eq!(merged.len(), 3);
    assert_eq!(merged[0].s, 0);
    assert_eq!(merged[1].s, 50);
    assert_eq!(merged[2].s, 70);
    for w in merged.windows(2) {
        assert!(w[1].s > w[0].e, "groupes non disjoints : {w:?}");
    }
}

#[test]
fn test_e3_fusion_containers() {
    // Un conteneur [0, 100] absorbe trois paires et un croisement inclus : le
    // groupe n'est qu'un réceptacle de candidats, jamais une anomalie en soi —
    // c'est l'É4 (fenêtres bornées) qui démêle.
    let cands = vec![
        cand_pair(0, 100),
        cand_pair(10, 20),
        cand_pair(30, 40),
        cand_cross(50, 60),
        cand_pair(150, 160),
    ];
    let merged = merge_candidates(&cands);
    assert_eq!(merged.len(), 2);
    assert_eq!((merged[0].s, merged[0].e), (0, 100));
    assert_eq!(merged[0].paires, vec![(0, 100), (10, 20), (30, 40)]);
    assert_eq!(merged[0].croisements, vec![(50, 60)]);
    assert_eq!((merged[1].s, merged[1].e), (150, 160));
}

// ─── Sous-étape 2.3 : É4a (raffinage des paires) ─────────────────────

#[test]
fn test_e4a_refine_pair() {
    // La paire brute (échantillons 0 et 30) héberge les points de référence P0
    // et P6, superposés par le re-parcours : le raffinage restitue la distance
    // réelle de 0,0 m, alors que la paire brute mesurait 30 m.
    let r = r_out_and_back();
    let (ref_x, ref_y) = out_and_back_ref();
    let raw = (r.rx[30] - r.rx[0]).hypot(r.ry[30] - r.ry[0]);
    assert_close(raw, 30.0);

    let merged = vec![group(vec![(0, 30)])];
    let refined = refine_pairs(&merged, &r.orig, &ref_x, &ref_y, r.n);
    assert_eq!(refined.len(), 1);
    assert_eq!((refined[0].a, refined[0].b), (0, 28));
    assert_close(refined[0].d, 0.0);
}

#[test]
fn test_e4a_dedup() {
    // Deux paires brutes distinctes — (0, 30) et (0, 29) — convergent vers le
    // même optimum raffiné (0, 28) : la clé `best.a·10⁶ + best.b` n'en conserve
    // qu'une seule occurrence.
    let r = r_out_and_back();
    let (ref_x, ref_y) = out_and_back_ref();
    let merged = vec![group(vec![(0, 30)]), group(vec![(0, 29)])];
    let refined = refine_pairs(&merged, &r.orig, &ref_x, &ref_y, r.n);
    assert_eq!(refined.len(), 1);
    assert_eq!((refined[0].a, refined[0].b), (0, 28));
}

#[test]
fn test_e4a_dmin() {
    // dMin est le minimum des distances RAFFINÉES (et vaut 0 en l'absence de
    // paire) : c'est le seuil d'éligibilité du coin de refermeture de l'É4b.
    let refined = vec![
        RefinedPair {
            a: 0,
            b: 10,
            d: 40.0,
        },
        RefinedPair {
            a: 20,
            b: 30,
            d: 5.0,
        },
        RefinedPair {
            a: 40,
            b: 50,
            d: 12.0,
        },
    ];
    assert_close(min_pair_distance(&refined), 5.0);
    assert_close(min_pair_distance(&[]), 0.0);
}

// ─── Sous-étape 2.4 : marches de cap et refermeture ──────────────────

#[test]
fn test_rp_walk_min_distance() {
    // Échantillons espacés de 10 m : la marche s'arrête au premier échantillon
    // qui atteint la portée demandée.
    let pts: Vec<(f64, f64)> = (0..=10).map(|i| (i as f64 * 10.0, 0.0)).collect();
    let r = r_from(&pts, 4.0);

    assert_eq!(rp_walk(&r, 0, 1, RP_WALK_MIN_M, RP_WALK_MAX_SEG), Some(1));
    assert_eq!(rp_walk(&r, 0, 1, 25.0, RP_WALK_MAX_SEG), Some(3));
    assert_eq!(rp_walk(&r, 10, -1, RP_WALK_MIN_M, RP_WALK_MAX_SEG), Some(9));
    // Portée inatteignable en 4 segments : repli sur le dernier échantillon
    // visité, la marche ayant tout de même parcouru une distance.
    assert_eq!(rp_walk(&r, 0, 1, 1000.0, RP_WALK_MAX_SEG), Some(4));
}

#[test]
fn test_rp_walk_micro_segments() {
    // Micro-segments (< 0,05 m) sautés ET hors budget : avec un budget d'un
    // seul segment, la marche traverse le micro-segment puis ressort au
    // deuxième échantillon (un micro-segment budgété l'aurait arrêtée au
    // premier).
    let pts: Vec<(f64, f64)> = vec![(0.0, 0.0), (0.01, 0.0), (10.01, 0.0), (20.01, 0.0)];
    let r = r_from(&pts, 4.0);
    assert_eq!(rp_walk(&r, 0, 1, RP_WALK_MIN_M, 1), Some(2));
    // Une seule distance parcourue, et elle est micro : repli `null`.
    assert_eq!(rp_walk(&r, 1, -1, RP_WALK_MIN_M, RP_WALK_MAX_SEG), None);
}

#[test]
fn test_rp_walk_bounds() {
    let pts: Vec<(f64, f64)> = (0..=10).map(|i| (i as f64 * 10.0, 0.0)).collect();
    let r = r_from(&pts, 4.0);

    // Bornes de trace : aucune distance parcourue → `null`.
    assert_eq!(rp_walk(&r, 0, -1, RP_WALK_MIN_M, RP_WALK_MAX_SEG), None);
    assert_eq!(rp_walk(&r, r.n, 1, RP_WALK_MIN_M, RP_WALK_MAX_SEG), None);
    // Le budget de segments borne la marche avant la borne de trace.
    assert_eq!(rp_walk(&r, r.n, -1, 1000.0, RP_WALK_MAX_SEG), Some(r.n - 4));
}

#[test]
fn test_closing_turn_uturn() {
    // Aller jusqu'au point 3 puis retour sur les pas : le coin de refermeture
    // mesure exactement un demi-tour (les deux côtés sont explorables).
    let pts: Vec<(f64, f64)> = vec![
        (0.0, 0.0),
        (10.0, 0.0),
        (20.0, 0.0),
        (30.0, 0.0),
        (20.0, 0.0),
        (10.0, 0.0),
        (0.0, 0.0),
    ];
    let r = r_from(&pts, 4.0);
    let t = closing_turn(&r, 3).expect("demi-tour mesurable des deux côtés");
    assert_close_tol(t, 180.0, 1e-9);
}

#[test]
fn test_closing_turn_straight() {
    // Ligne droite : cap entrant et cap sortant identiques → 0°.
    let pts: Vec<(f64, f64)> = (0..=10).map(|i| (i as f64 * 10.0, 0.0)).collect();
    let r = r_from(&pts, 4.0);
    let t = closing_turn(&r, 5).expect("les deux côtés sont explorables");
    assert_close_tol(t, 0.0, 1e-9);

    // Un côté inexplorable (borne de trace) → mesure indéterminable.
    let r2 = r_from(&[(0.0, 0.0), (10.0, 0.0)], 4.0);
    assert!(closing_turn(&r2, 0).is_none());
}

#[test]
fn test_closing_delta_straight() {
    // Fenêtre prise sur une ligne droite : la trace repart dans son cap
    // d'entrée, donc dc = 0° — la fenêtre ne sera PAS « propre ».
    let pts: Vec<(f64, f64)> = (0..=20).map(|i| (i as f64 * 10.0, 0.0)).collect();
    let r = r_from(&pts, 4.0);
    let dc = closing_delta(&r, 4, 12).expect("les deux côtés sont explorables");
    assert_close_tol(dc, 0.0, 1e-9);
}

#[test]
fn test_closing_delta_turn() {
    // Entrée vers l'est, sortie vers le nord : écart de cap de 90°.
    let mut pts: Vec<(f64, f64)> = (0..=5).map(|i| (i as f64 * 10.0, 0.0)).collect();
    for i in 1..=5 {
        pts.push((50.0, i as f64 * 10.0));
    }
    let r = r_from(&pts, 4.0);
    let dc = closing_delta(&r, 4, 8).expect("les deux côtés sont explorables");
    assert_close_tol(dc, 90.0, 1e-9);
}

// ─── Sous-étape 2.4 : anti-aiguille (loop_degenerate) ────────────────

#[test]
fn test_loop_degenerate_uturn() {
    // Aller-retour exact : aire algébrique nulle, donc circularité ≈ 0 — le
    // test 1 suffit à rejeter le motif.
    let pts: Vec<(f64, f64)> = vec![
        (0.0, 0.0),
        (10.0, 0.0),
        (20.0, 0.0),
        (30.0, 0.0),
        (20.0, 0.0),
        (10.0, 0.0),
        (0.0, 0.0),
    ];
    let r = r_from(&pts, 4.0);
    assert!(loop_degenerate(&r, 0, r.n, 15.0));
}

#[test]
fn test_loop_degenerate_circle() {
    // Cercle : circularité ≈ 1 et miroirs écartés en 2r·sin(θ/2) → motif
    // conservé (fenêtre publiable).
    let r = circle(120.0, 75);
    assert!(!loop_degenerate(&r, 0, r.n, 15.0));
}

#[test]
fn test_loop_degenerate_partial_mirror() {
    // Aiguille à écart latéral de 2 m (< close_thr) : la circularité ≈ 0,18
    // passe le test 1, mais la fraction miroir vaut 1,0 ≥ 0,95 → rejet par le
    // test 2 (les deux tests sont indépendants).
    let pts: Vec<(f64, f64)> = vec![
        (0.0, 0.0),
        (10.0, 0.0),
        (20.0, 0.0),
        (30.0, 0.0),
        (30.0, 2.0),
        (20.0, 2.0),
        (10.0, 2.0),
        (0.0, 2.0),
    ];
    let r = r_from(&pts, 4.0);
    assert!(loop_degenerate(&r, 0, r.n, 15.0));

    // Épingle à re-parcours PARTIEL : écart latéral de 20 m (> close_thr), donc
    // miroirs non superposés (fraction 0) et circularité ≈ 0,44 — la fenêtre
    // est publiée, à charge de l'hôte de la traiter (limite assumée §15.3).
    let pts2: Vec<(f64, f64)> = vec![
        (0.0, 0.0),
        (25.0, 0.0),
        (50.0, 0.0),
        (75.0, 0.0),
        (100.0, 0.0),
        (100.0, 20.0),
        (75.0, 20.0),
        (50.0, 20.0),
        (25.0, 20.0),
        (0.0, 20.0),
    ];
    let r2 = r_from(&pts2, 4.0);
    assert!(!loop_degenerate(&r2, 0, r2.n, 15.0));
}

// ─── Sous-étape 2.4 : fenêtres bornées (É4b/c/d/e) ───────────────────

#[test]
fn test_e4_window_lmax() {
    // Périmètre de 817 m > RP_LMAX (800 m) : fenêtre rejetée avant même le test
    // angulaire — c'est le bornage qui écarte les méga-paires.
    let r_big = circle(130.0, 82);
    let pre_big = compute_heading_prefix(&r_big);
    let c_big = WindowCtx::new(&r_big, &pre_big, 15.0, 0.0, 270.0);
    let mut w_big = Vec::new();
    push_window(&c_big, 0, r_big.n, None, false, &mut w_big);
    assert!(w_big.is_empty(), "fenêtre non bornée publiée");

    // Même rotation, périmètre de 754 m : la fenêtre est publiée.
    let r_ok = circle(120.0, 75);
    let pre_ok = compute_heading_prefix(&r_ok);
    let c_ok = WindowCtx::new(&r_ok, &pre_ok, 15.0, 0.0, 270.0);
    let mut w_ok = Vec::new();
    push_window(&c_ok, 0, r_ok.n, None, false, &mut w_ok);
    assert_eq!(w_ok.len(), 1);
}

#[test]
fn test_e4_window_quantification() {
    let r = circle(120.0, 75);
    let pre = compute_heading_prefix(&r);
    let c = WindowCtx::new(&r, &pre, 15.0, 0.0, 270.0);

    // Tour complet : rotation brute 350,4° — alignée sur 360° exactement.
    let mut w = Vec::new();
    push_window(&c, 0, r.n, None, false, &mut w);
    assert_eq!(w.len(), 1);
    assert_close(w[0].total, 360.0);

    // Arc de 312° : rotation brute 307,2°, hors de la fenêtre de ±40° — la
    // valeur brute est conservée (et reste au-dessus du seuil de 270°).
    let mut w2 = Vec::new();
    push_window(&c, 0, 65, None, false, &mut w2);
    assert_eq!(w2.len(), 1);
    assert_close_tol(w2[0].total, 307.2, 1e-6);
}

#[test]
fn test_e4_window_propre() {
    // Voie d'entrée/sortie commune : la trace ressort dans son cap d'entrée
    // (dc = 0°) → fenêtre NON propre, malgré un tour complet mesuré à 360°.
    let (r, a, b) = roundabout(0.0);
    let pre = compute_heading_prefix(&r);
    let c = WindowCtx::new(&r, &pre, 15.0, 0.0, 270.0);
    let mut w = Vec::new();
    push_window(&c, a, b, Some(0.0), true, &mut w);
    assert_eq!(w.len(), 1);
    assert_close(w[0].total, 360.0);
    assert!(!w[0].propre, "voie commune : fenêtre non propre attendue");

    // Sortie perpendiculaire : |dc| = 90° ≥ 60° → fenêtre propre. La rotation
    // mesurée déborde au-delà de 360° (le cap de sortie entre dans le cumul) :
    // c'est le motif de la fenêtre « débordante » que l'É4e écarte.
    let (r2, a2, b2) = roundabout(90.0);
    let pre2 = compute_heading_prefix(&r2);
    let c2 = WindowCtx::new(&r2, &pre2, 15.0, 0.0, 270.0);
    let mut w2 = Vec::new();
    push_window(&c2, a2, b2, Some(0.0), true, &mut w2);
    assert_eq!(w2.len(), 1);
    assert!(w2[0].propre);
    assert!(w2[0].total > 360.0, "rotation débordante attendue");

    // Croisement (withCoin = false) : le drapeau propre est toujours vrai.
    let mut w3 = Vec::new();
    push_window(&c, a, b, None, false, &mut w3);
    assert_eq!(w3.len(), 1);
    assert!(w3[0].propre);
}

#[test]
fn test_e4_greedy_selection() {
    // Glouton : propre d'abord, puis rotation la plus forte, puis refermeture
    // la plus serrée, puis fenêtre la plus compacte ; les retenues sont
    // disjointes et re-triées par `a` croissant.
    let taken = select_windows(vec![
        window(0, 100, Some(0.0), 300.0, false),
        window(10, 90, Some(0.0), 360.0, true),
        window(20, 80, Some(0.0), 360.0, false),
        window(200, 300, Some(5.0), 360.0, false),
    ]);
    assert_eq!(taken.len(), 2);
    assert_eq!((taken[0].a, taken[0].b), (10, 90));
    assert_eq!((taken[1].a, taken[1].b), (200, 300));

    // À propreté et rotation égales, la refermeture la plus serrée passe
    // d'abord (`None` vaut ∞) ; les deux autres, chevauchantes, sont écartées.
    let taken_d = select_windows(vec![
        window(0, 100, None, 360.0, true),
        window(0, 100, Some(2.0), 360.0, true),
        window(0, 100, Some(20.0), 360.0, true),
    ]);
    assert_eq!(taken_d.len(), 1);
    assert_close(taken_d[0].d.unwrap(), 2.0);

    // À tout le reste égal, la fenêtre la plus compacte l'emporte.
    let taken_span = select_windows(vec![
        window(0, 200, Some(1.0), 360.0, true),
        window(0, 100, Some(1.0), 360.0, true),
    ]);
    assert_eq!(taken_span.len(), 1);
    assert_eq!(taken_span[0].b, 100);
}

#[test]
fn test_e4d_pairs_in() {
    // É4d : chaque fenêtre ne retient que les paires raffinées CONTENUES dans
    // son intervalle (balayage trié, coupure dès que `q.a > w.b`).
    let mut windows = vec![window(10, 50, Some(0.0), 360.0, true)];
    let refined = vec![
        RefinedPair {
            a: 0,
            b: 40,
            d: 1.0,
        }, // commence avant la fenêtre
        RefinedPair {
            a: 10,
            b: 50,
            d: 2.0,
        }, // contenue (bornes incluses)
        RefinedPair {
            a: 20,
            b: 45,
            d: 3.0,
        }, // contenue
        RefinedPair {
            a: 30,
            b: 60,
            d: 4.0,
        }, // dépasse la fenêtre
    ];
    fill_pairs_in(&mut windows, &refined);

    let pairs: Vec<(usize, usize)> = windows[0]
        .pairs_in
        .iter()
        .map(|p| (p.a, p.b))
        .collect();
    assert_eq!(pairs, vec![(10, 50), (20, 45)]);
}

// ─── Sous-étape 2.5 : ancres d'accès ─────────────────────────────────

#[test]
fn test_anchor_fit_circle() {
    // Cercle de rayon 20 centré en (100, 50) : Kåsa le restitue exactement sur
    // des points parfaits.
    let samples: Vec<(f64, f64)> = (0..40)
        .map(|i| {
            let t = i as f64 * 2.0 * PI / 40.0;
            (100.0 + 20.0 * t.cos(), 50.0 + 20.0 * t.sin())
        })
        .collect();
    let (cx, cy, r) = fit_circle_kasa(&samples).expect("ajustement adopté");
    assert_close_tol(cx, 100.0, 1e-6);
    assert_close_tol(cy, 50.0, 1e-6);
    assert_close_tol(r, 20.0, 1e-6);

    // Moins de trois points : ajustement impossible.
    assert!(fit_circle_kasa(&[(0.0, 0.0), (1.0, 0.0)]).is_none());
    // Points confondus : système singulier (déterminant nul).
    let degenere = [(5.0, 5.0); 6];
    assert!(fit_circle_kasa(&degenere).is_none());
}

#[test]
fn test_anchor_median_radius() {
    // Le rayon publié est la MÉDIANE des distances au centre : sur un cercle
    // parfait, elle vaut le rayon du cercle.
    let (px, py) = loop_only(20.0, 40);
    let an = rp_anchor_indices(&px, &py, 0, 40, 15.0);
    assert_close_tol(an.r.expect("rayon publié"), 20.0, 1e-9);
    assert_close_tol(an.cx.expect("centre publié"), 0.0, 1e-9);
    assert_close_tol(an.cy.expect("centre publié"), 20.0, 1e-9);

    // Cœur entièrement confondu : médiane nulle, repli à 1 m (`|| 1` du JS).
    let pts = vec![0.0; 20];
    let an2 = rp_anchor_indices(&pts, &pts, 0, 19, 15.0);
    assert_close_tol(an2.r.expect("rayon de repli"), 1.0, 1e-9);
}

#[test]
fn test_anchor_walk_radial() {
    // Approche rectiligne de 200 m, giratoire de rayon 20 m refermé en (0, 0),
    // sortie rectiligne : le disque élargi (r + 12 = 32 m) est franchi à 30 m
    // du centre, la sortie est confirmée au point suivant, puis l'extension de
    // 15 m amène l'ancre 40 m à l'écart — sur la route franche, pas dans la
    // zone anormale.
    let (px, py, junc, ce) = roundabout_axes();
    let an = rp_anchor_indices(&px, &py, junc, ce, 15.0);
    assert_eq!(an.up, Some(16));
    assert_eq!(an.dn, Some(64));
    assert_close_tol(an.r.expect("rayon"), 20.0, 1e-6);
    assert_close_tol(an.cx.expect("centre"), 0.0, 1e-6);
    assert_close_tol(an.cy.expect("centre"), 20.0, 1e-6);
}

#[test]
fn test_anchor_walk_persist() {
    // Une excursion hors zone d'UN SEUL échantillon (aberrant GPS) ne confirme
    // pas la sortie : RP_ANCHOR_PERSIST points consécutifs sont requis. L'ancre
    // aval reste donc sur la route, au-delà du vrai point de sortie.
    let (px, py, junc, ce) = roundabout_axes_with_outlier();
    let an = rp_anchor_indices(&px, &py, junc, ce, 15.0);
    assert_eq!(an.dn, Some(65));
    // Le point aberrant (index 62) est bien hors zone : c'est la persistance
    // qui l'écarte, pas le test de zone.
    let d_aberant = ((px[62] - an.cx.unwrap()).powi(2) + (py[62] - an.cy.unwrap()).powi(2)).sqrt();
    assert!(d_aberant > an.r.unwrap() + 12.0);
}

#[test]
fn test_anchor_bord_de_trace() {
    // Boucle en TÊTE de trace : la sortie de zone coïncide avec la borne de
    // trace, ce qui vaut confirmation en soi (cas C1) — l'ancre amont est le
    // premier échantillon.
    let (px, py) = loop_with_lead_out_of_zone();
    let an = rp_anchor_indices(&px, &py, 1, 41, 15.0);
    assert_eq!(an.up, Some(0));
}

#[test]
fn test_anchor_repli_silencieux() {
    // Boucle en tête de trace sans aucun point hors zone en amont : aucune
    // ancre amont n'est déterminable → `None` (l'appelant retombe sur
    // `junc − 1`), silencieusement, sans panique.
    let (mut px, mut py) = loop_only(20.0, 40);
    for i in 1..=20 {
        px.push(10.0 * i as f64);
        py.push(0.0);
    }
    let an = rp_anchor_indices(&px, &py, 0, 40, 15.0);
    assert!(an.up.is_none());
    assert!(an.dn.is_some());
}

#[test]
fn test_anchor_hors_zone_distance() {
    // Le garde de portée porte sur la distance HORS ZONE : un re-parcours
    // superposé de 150 m à l'intérieur du disque (allers-retours entre (10, 0)
    // et (−10, 0)) n'épuise pas la portée de 150 m, l'ancre aval est trouvée.
    let mut px = Vec::new();
    let mut py = Vec::new();
    for i in 0..=40 {
        let t = i as f64 * 2.0 * PI / 40.0;
        px.push(20.0 * t.sin());
        py.push(20.0 - 20.0 * t.cos());
    }
    for k in 0..8 {
        px.push(if k % 2 == 0 { 10.0 } else { -10.0 });
        py.push(0.0);
    }
    for i in 1..=3 {
        px.push(10.0 * (4 + i) as f64); // 50, 60, 70
        py.push(0.0);
    }
    let an = rp_anchor_indices(&px, &py, 0, 40, 15.0);
    assert_eq!(an.dn, Some(50));
}

// ─── Sous-étape 2.5 : É5 (publication) ───────────────────────────────

#[test]
fn test_e5_convert_indices() {
    // Toute grandeur publiée transite par `R.orig` : aucune valeur d'échantillon
    // n'est publiée, et aucun index de référence n'atteint `m`.
    let (r, px, py, ids) = e5_fixture();
    let mut w = window(0, r.n, Some(0.0), 360.0, true);
    w.pairs_in = vec![RefinedPair {
        a: 10,
        b: 20,
        d: 3.0,
    }];

    let findings = publish_windows(&[w], &r, &px, &py, &ids, 15.0);
    assert_eq!(findings.len(), 1);
    let f = &findings[0];

    assert_eq!(f.kind, FindingKind::Rp);
    assert_eq!(f.id, "rp-1");
    assert_eq!(f.label, "Tour de rond-point : 1");
    assert_eq!(f.total_angle, Some(360));
    assert_eq!(f.turn_text.as_deref(), Some("tour complet (1×)"));

    // Emprise et cœur : toute la trace (les deux ancres sont indéterminables
    // aux bornes → repli sur les valeurs bornées).
    assert_eq!((f.parts[0].s, f.parts[0].e), (0, 59));
    assert_eq!((f.parts[1].s, f.parts[1].e), (0, 59));
    assert_eq!(f.zone_ids.len(), 60);
    assert_eq!(f.core_ids.len(), 60);
    assert_eq!(f.ctx.up, None);
    assert_eq!(f.ctx.dn, None);
    assert_eq!(f.peak, 0);
    assert_eq!(f.peak_id, 1000);

    // Paire convertie : échantillons (10, 20) → références (5, 10).
    assert_eq!(f.pairs.len(), 1);
    assert_eq!((f.pairs[0].a, f.pairs[0].b), (5, 10));
    assert_eq!((f.pairs[0].aid, f.pairs[0].bid), (1005, 1010));
    assert_close(f.pairs[0].d, 3.0);
    assert_eq!(f.pair_idx, vec![5, 10]);

    assert_eq!(f.status, FindingStatus::Pending);
    assert!(f.correction.is_none() && f.undo.is_none());
    assert!(f.ecart.is_none() && f.d1.is_none() && f.d2.is_none());
}

#[test]
fn test_e5_pairs_top_6() {
    // Seules les RP_PAIRS_TOP paires les plus serrées sont publiées, triées par
    // `d` croissant puis converties en indices de référence.
    let (r, px, py, ids) = e5_fixture();
    let mut w = window(0, r.n, Some(0.0), 360.0, true);
    w.pairs_in = (0..8)
        .map(|k| RefinedPair {
            a: 2 * k + 10,
            b: 2 * k + 40,
            d: 8.0 - k as f64,
        })
        .collect();

    let findings = publish_windows(&[w], &r, &px, &py, &ids, 15.0);
    let f = &findings[0];
    assert_eq!(f.pairs.len(), RP_PAIRS_TOP);
    let ds: Vec<f64> = f.pairs.iter().map(|p| p.d).collect();
    assert_eq!(ds, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    // a = 2k+10 → k+5 et b = 2k+40 → k+20 ; les six paires retenues sont
    // d = 1..6, soit k = 7..2.
    assert_eq!((f.pairs[0].a, f.pairs[0].b), (12, 27));
    assert_eq!(f.pairs[0].aid, 1012);
    assert_eq!((f.pairs[5].a, f.pairs[5].b), (7, 22));
    assert_eq!(f.pair_idx.len(), 12);
}

#[test]
fn test_e5_junction_is_start() {
    // La jonction publiée (`peak`) est le DÉBUT de boucle — `orig[w.a]` — et
    // non le milieu de la paire de refermeture (qui tomberait au centre du
    // giratoire quand les voies d'entrée et de sortie sont superposées).
    let (r, px, py, ids) = e5_fixture();
    let w = window(20, 80, Some(4.0), 270.0, false);

    let findings = publish_windows(&[w], &r, &px, &py, &ids, 15.0);
    let f = &findings[0];
    assert_eq!(f.peak, 10);
    assert_eq!(f.peak_id, ids[10]);
    assert_ne!(f.peak, (20 + 80) / 2);
    assert_eq!(f.parts[1].s, 10);
    assert_eq!(f.total_angle, Some(270));
    assert_eq!(f.label, "Boucle giratoire : 1");
    assert_eq!(f.parts[0].text, "refermeture par croisement de trace");
}

// ─── Scénarios de validation §17.2 ───────────────────────────────────

/// Exécute le pipeline RP complet (jusqu'à la sélection gloutonne) sur un
/// fichier de test : permet de vérifier des propriétés de **fenêtre** que le
/// finding ne publie pas (drapeau `propre`, bornage par `RP_LMAX`).
fn run_rp_pipeline(prefix: &str) -> (ResampledGeo, Vec<Window>) {
    let raw = load_gpx(prefix);
    let params = rp_params();
    let (kept, _removed) = consolidate_points(&raw, params.consol_m);
    let geo = build_geometry(&kept);
    let r = resample_geo(&geo.px, &geo.py, &geo.cum, geo.total, RP_STEP_DEFAULT);

    let close_thr = params.close_m;
    let pre = compute_heading_prefix(&r);
    let mut cands = find_proximity_candidates(&r, close_thr);
    cands.extend(find_crossing_candidates(&r, close_thr));
    let merged = merge_candidates(&cands);
    let refined = refine_pairs(&merged, &r.orig, &geo.px, &geo.py, r.n);
    let d_min = min_pair_distance(&refined);

    let ctx = WindowCtx::new(&r, &pre, close_thr, d_min, params.angle_deg as f64);
    let mut windows = Vec::new();
    for p in &refined {
        push_window(&ctx, p.a, p.b, Some(p.d), true, &mut windows);
    }
    for g in &merged {
        for &(ca, cb) in &g.croisements {
            push_window(&ctx, ca, cb, None, false, &mut windows);
        }
    }
    fill_pairs_in(&mut windows, &refined);
    let taken = select_windows(windows);
    (r, taken)
}

/// Paires publiées sous forme `(a, b, d arrondie à 0,1 m)`.
fn pairs_brief(f: &Finding) -> Vec<(usize, usize, f64)> {
    f.pairs
        .iter()
        .map(|p| (p.a, p.b, (p.d * 10.0).round() / 10.0))
        .collect()
}

#[test]
fn test_scenario_rondpoints_g1() {
    // Tour complet + branches communes : 360°, fenêtre PROPRE, paires 0,0 m.
    let findings = detect_scenario("rondpoints-g1");
    assert_eq!(findings.len(), 1);
    let f = &findings[0];
    assert_eq!(f.kind, FindingKind::Rp);
    assert_eq!(f.label, "Tour de rond-point : 1");
    assert_eq!(f.total_angle, Some(360));
    assert_eq!(f.turn_text.as_deref(), Some("tour complet (1×)"));
    assert_eq!(f.peak, 4);
    assert_eq!((f.parts[0].s, f.parts[0].e), (1, 38));
    assert_eq!((f.parts[1].s, f.parts[1].e), (4, 33));
    assert_eq!((f.ctx.up, f.ctx.dn), (Some(0), Some(39)));
    // Les paires de superposition exacte (0,0 m) sont bien restituées.
    assert_eq!(f.pairs.len(), 4);
    assert_eq!(
        pairs_brief(f),
        vec![(4, 33, 0.0), (4, 33, 0.0), (4, 33, 0.0), (4, 32, 5.3)]
    );

    // Fenêtre retenue : propre, bornée, et son cœur est bien la boucle publiée.
    let (r, taken) = run_rp_pipeline("rondpoints-g1");
    assert_eq!(taken.len(), 1);
    assert!(taken[0].propre, "fenêtre propre attendue");
    let span = r.cum_r[taken[0].b] - r.cum_r[taken[0].a];
    assert!(span <= 800.0, "fenêtre non bornée : {span:.1} m");
}

#[test]
fn test_scenario_rondpoints_g2() {
    // Tour complet : 360°.
    let findings = detect_scenario("rondpoints-g2");
    assert_eq!(findings.len(), 1);
    let f = &findings[0];
    assert_eq!(f.label, "Tour de rond-point : 1");
    assert_eq!(f.total_angle, Some(360));
    assert_eq!(f.turn_text.as_deref(), Some("tour complet (1×)"));
    assert_eq!(f.peak, 2);
    assert_eq!((f.parts[0].s, f.parts[0].e), (0, 33));
    assert_eq!((f.parts[1].s, f.parts[1].e), (2, 21));
    assert_eq!((f.ctx.up, f.ctx.dn), (None, Some(34)));
    assert_eq!(pairs_brief(f), vec![(2, 21, 10.4)]);
}

#[test]
fn test_scenario_rondpoints_g3() {
    // Refermeture non exacte (~12 m) : la rotation brute est hors de 360° mais
    // la quantification ±40° l'aligne — verdict publié : 360°.
    let findings = detect_scenario("rondpoints-g3");
    assert_eq!(findings.len(), 1);
    let f = &findings[0];
    assert_eq!(f.total_angle, Some(360));
    assert_eq!(f.turn_text.as_deref(), Some("tour complet (1×)"));
    assert_eq!(f.peak, 5);
    assert_eq!((f.parts[0].s, f.parts[0].e), (0, 25));
    assert_eq!((f.parts[1].s, f.parts[1].e), (5, 23));
    assert_eq!((f.ctx.up, f.ctx.dn), (None, Some(26)));
    assert_eq!(pairs_brief(f), vec![(5, 23, 0.0)]);
}

#[test]
fn test_scenario_rp_santa_susanna() {
    // Tour complet + réengagement de la branche d'entrée : le lissage ±1
    // écrase le demi-tour (≈175° mesurés), `closingTurn` restitue le coin et le
    // verdict est 360°.
    let findings = detect_scenario("RP-Santa");
    assert_eq!(findings.len(), 1);
    let f = &findings[0];
    assert_eq!(f.label, "Tour de rond-point : 1");
    assert_eq!(f.total_angle, Some(360));
    assert_eq!(f.turn_text.as_deref(), Some("tour complet (1×)"));
    assert_eq!(f.peak, 2);
    assert_eq!((f.parts[0].s, f.parts[0].e), (0, 24));
    assert_eq!((f.parts[1].s, f.parts[1].e), (2, 16));
    assert_eq!((f.ctx.up, f.ctx.dn), (None, None));
    assert_eq!(
        pairs_brief(f),
        vec![
            (2, 16, 0.0),
            (2, 16, 0.0),
            (2, 16, 0.0),
            (2, 16, 0.0),
            (2, 16, 0.0),
            (2, 15, 10.0)
        ]
    );
}

#[test]
fn test_scenario_ar_santa_susanna_rejected() {
    // Aiguille (aller-retour exact) : rejetée par l'anti-aiguille du RP —
    // circularité ≈ 0 (aire algébrique nulle) et miroir ≈ 1,0.
    let findings = detect_scenario("AR-Santa");
    assert!(findings.is_empty(), "aiguille publiée par RP : {findings:?}");
    let (_, taken) = run_rp_pipeline("AR-Santa");
    assert!(taken.is_empty(), "fenêtre retenue pour une aiguille");
}

#[test]
fn test_scenario_ar_detecte_aussi_en_rp() {
    // Aiguille ambiguë : rejetée par le RP (aucun candidat ne franchit
    // l'anti-aiguille), captée par le détecteur AR.
    let findings = detect_scenario("AR_Detecte");
    assert!(findings.is_empty(), "aiguille ambiguë publiée par RP");
}

#[test]
fn test_scenario_rp_erreur_magny_4_turns() {
    // Aller-retour macroscopique (4,3 km) + giratoire : une seule anomalie,
    // fenêtre BORNÉE au giratoire (la méga-paire de ~4 km est écartée par
    // RP_LMAX), drapeau propre.
    //
    // Le fichier porte un aller-retour macroscopique (4,3 km) et un giratoire
    // réel à 4 tours. Le verdict « 1440° — 4 tours complets » annoncé par la
    // version 1.0 de l'ANALYSE (§17.2) n'est pas reproductible : la référence
    // GELÉE (`verifgpx-V3.0.html`, extraite verbatim et exécutée sur ce même
    // fichier) produit « 776° — 2 tours complets + 1/4 de tour » avec la fenêtre
    // (231, 275), et le portage Rust produit exactement les mêmes valeurs,
    // intermédiaires compris (cands=2688, merged=1, refined=665, dMin=0,00,
    // windows=182, taken=1). La spécification a été amendée en conséquence
    // (ANALYSE §18, avenant du 2026-09-11) ; le Livrable 8 s'aligne par avenant.
    // Les assertions ci-dessous décrivent donc le comportement de référence.
    let findings = detect_scenario("RP_erreur_Magny");
    assert_eq!(findings.len(), 1);
    let f = &findings[0];
    assert_eq!(f.label, "Tour de rond-point : 1");
    assert_eq!(f.total_angle, Some(776));
    assert_eq!(f.turn_text.as_deref(), Some("2 tours complets + 1/4 de tour"));
    assert_eq!(f.peak, 39);
    assert_eq!((f.parts[0].s, f.parts[0].e), (35, 65));
    assert_eq!((f.parts[1].s, f.parts[1].e), (39, 61));
    assert_eq!((f.ctx.up, f.ctx.dn), (Some(34), Some(66)));
    assert_eq!(
        pairs_brief(f),
        vec![
            (39, 49, 0.0),
            (39, 49, 0.0),
            (39, 49, 0.0),
            (39, 59, 0.0),
            (39, 59, 0.0),
            (39, 59, 0.0)
        ]
    );

    // Fenêtre bornée au giratoire : ~173 m de périmètre, très en deçà de
    // RP_LMAX (800 m) — c'est le bornage qui écarte la méga-paire.
    let (r, taken) = run_rp_pipeline("RP_erreur_Magny");
    assert_eq!(taken.len(), 1);
    assert!(taken[0].propre);
    let span = r.cum_r[taken[0].b] - r.cum_r[taken[0].a];
    assert!(
        span <= 800.0,
        "fenêtre non bornée : {span:.1} m (RP_LMAX = 800 m)"
    );
}

