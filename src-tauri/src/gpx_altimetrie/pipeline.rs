//! Pipeline altimétrique — portage du prototype validé
//! (`features/altimetrie/prototype/index.html`, GPX Climb Analyzer v3.9.6).
//!
//! Le module est **pur** : il ne dépend ni de Tauri ni du système de fichiers.
//! Les commandes lui fournissent les points GPX et les paramètres ; il rend un
//! [`AltimetrieFile`] complet.
//!
//! Écart assumé par rapport au prototype : seules les extrémités de la trace
//! sont ancrées sur l'altitude brute. Ancrer aussi les bornes des montées
//! recréait un pic ou un creux d'un point là où le lissage avait aplati le
//! relief, et faussait la pente maximale.
//!
//! Enchaînement :
//! 1. distances cumulées 2D (Haversine) et correction des plateaux ;
//! 2. rééchantillonnage par moyenne, puis anti-pic Hampel ;
//! 3. lissage (Savitzky-Golay, gaussien), pentes et catégories ;
//! 4. détection par hystérésis, meilleur sous-segment, raffinement, fusion ;
//! 5. `rebuild` : contraintes d'édition, profil lissé, segments, métriques.
//!
//! Les autres choix numériques du prototype sont conservés, y compris ceux qui
//! paraissent surprenants : Savitzky-Golay toujours d'ordre 2, fenêtre de pente
//! pouvant atteindre le double du réglage, anti-pic avec répétition des bords,
//! sous-segment **le plus long** et non le plus pentu, pente d'une montée
//! calculée en D+/longueur.

use std::collections::BTreeSet;

use super::types::{
    AltimetrieCategoryDef, AltimetrieClimb, AltimetrieClimbEdit, AltimetrieFile,
    AltimetrieLegend, AltimetrieMergedPart, AltimetrieMergedPartEdit, AltimetrieParams,
    AltimetrieProfilePoint, AltimetrieSegment, AltimetrieTrace, FILE_VERSION,
};

const EARTH_RADIUS_M: f64 = 6_371_000.0;
const SLOPE_DISPLAY_MAX_PCT: f64 = 25.0;
const REFINE_MIN_M: f64 = 1_500.0;
const REFINE_MAX_M: f64 = 8_000.0;
const PLATEAU_TOLERANCE_M: f64 = 1.0;
const RESAMPLE_AVG_MIN_M: f64 = 75.0;

/// Catégories de pente, bornes hautes exclusives. `None` représente l'infini.
const CATEGORY_DEFS: [(&str, &str, Option<f64>, Option<f64>); 8] = [
    ("dh3", "Descente raide", None, Some(-7.0)),
    ("dh2", "Descente", Some(-7.0), Some(-3.0)),
    ("dh1", "Faux plat descendant", Some(-3.0), Some(-1.0)),
    ("pl", "Plat", Some(-1.0), Some(1.0)),
    ("up1", "Faux plat montant", Some(1.0), Some(3.0)),
    ("up2", "Montée", Some(3.0), Some(7.0)),
    ("up3", "Montée raide", Some(7.0), Some(10.0)),
    ("up4", "Montée très raide", Some(10.0), None),
];

const LEGEND_NOTE: &str = "Bornes hautes exclusives. Les montées d'un groupe séparé partagent le même groupId et sont listées individuellement ; une montée fusionnée non séparée porte isMerged=true et mergedParts. Les distances sont 2D (Haversine) et les progressions sont normalisées par la longueur réelle de la trace.";

/// Point GPX minimal utilisé par le pipeline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawPoint {
    pub lat: f64,
    pub lon: f64,
    pub ele: f64,
}

/// Trace préparée : points bruts, distances et profil rééchantillonné.
#[derive(Debug, Clone)]
struct PreparedTrace {
    points: Vec<RawPoint>,
    dist_raw: Vec<f64>,
    dist: Vec<f64>,
    ele_raw: Vec<f64>,
    total_dist_raw: f64,
    total_dist_sampled: f64,
    trailing_gap_m: f64,
    step_m: f64,
}

#[derive(Debug, Clone)]
struct Resampled {
    dist: Vec<f64>,
    ele: Vec<f64>,
}

#[derive(Debug, Clone)]
struct BoundsKm {
    start_km: f64,
    end_km: f64,
}

/// Montée issue de la détection automatique, avant conversion en état éditable.
#[derive(Debug, Clone)]
struct AutoClimb {
    start_km: f64,
    end_km: f64,
    merged_parts: Option<Vec<BoundsKm>>,
    fallback: bool,
}

#[derive(Debug, Clone)]
struct SubClimb {
    start_idx: usize,
    end_idx: usize,
    length: f64,
}

#[derive(Debug, Clone)]
struct Candidate {
    start_idx: usize,
    end_idx: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventKind {
    Start,
    Summit,
}

#[derive(Debug, Clone)]
struct Event {
    idx: usize,
    kind: EventKind,
}

#[derive(Debug, Clone)]
struct RawSegment {
    start_idx: usize,
    end_idx: usize,
    key: &'static str,
}

#[derive(Debug, Clone)]
struct RefinedClimb {
    start_idx: usize,
    end_idx: usize,
    merged_parts: Option<Vec<BoundsKm>>,
}

struct Rebuilt {
    display_ele: Vec<f64>,
    segments: Vec<AltimetrieSegment>,
    climbs: Vec<AltimetrieClimb>,
}

// ─── Entrées publiques ────────────────────────────────────────────────

/// Détecte les montées automatiquement et construit le fichier complet.
pub fn detect(
    points: &[RawPoint],
    params: &AltimetrieParams,
    trace_id: &str,
    source: &str,
    updated_at: &str,
) -> Result<AltimetrieFile, String> {
    let prepared = prepare(points, params)?;
    let auto_climbs = compute_auto(&prepared, params);
    let edits: Vec<AltimetrieClimbEdit> = auto_climbs
        .iter()
        .enumerate()
        .map(|(index, climb)| auto_to_edit(climb, index as u32 + 1))
        .collect();
    build_file_from_prepared(&prepared, params, trace_id, source, updated_at, &edits)
}

/// Reconstruit le fichier à partir d'une liste de montées éditée.
///
/// `updated_at` est fourni par l'appelant : un aperçu conserve l'horodatage du
/// fichier existant, une sauvegarde le remplace.
pub fn build_file(
    points: &[RawPoint],
    params: &AltimetrieParams,
    trace_id: &str,
    source: &str,
    updated_at: &str,
    climbs: &[AltimetrieClimbEdit],
) -> Result<AltimetrieFile, String> {
    let prepared = prepare(points, params)?;
    build_file_from_prepared(&prepared, params, trace_id, source, updated_at, climbs)
}

fn build_file_from_prepared(
    prepared: &PreparedTrace,
    params: &AltimetrieParams,
    trace_id: &str,
    source: &str,
    updated_at: &str,
    climbs: &[AltimetrieClimbEdit],
) -> Result<AltimetrieFile, String> {
    let rebuilt = rebuild(prepared, params, climbs);
    let profile = build_profile(prepared, &rebuilt.display_ele);

    Ok(AltimetrieFile {
        version: FILE_VERSION,
        trace_id: trace_id.to_string(),
        source: source.to_string(),
        updated_at: updated_at.to_string(),
        params: params.clone(),
        trace: AltimetrieTrace {
            total_distance_m: round_to(prepared.total_dist_raw, 1),
            total_distance_sampled_m: round_to(prepared.total_dist_sampled, 1),
            trailing_gap_m: round_to(prepared.trailing_gap_m, 1),
            step_m: prepared.step_m,
            total_points: profile.len(),
        },
        profile,
        segments: rebuilt.segments,
        climbs: rebuilt.climbs,
        legend: build_legend(params),
    })
}

fn build_legend(params: &AltimetrieParams) -> AltimetrieLegend {
    AltimetrieLegend {
        note: LEGEND_NOTE.to_string(),
        fenetre_calcul: params.fenetre_calcul,
        longueur_min: params.longueur_min,
        categories: CATEGORY_DEFS
            .iter()
            .map(|(id, label, min_pct, max_pct)| AltimetrieCategoryDef {
                id: id.to_string(),
                label: label.to_string(),
                min_pct: *min_pct,
                max_pct: *max_pct,
            })
            .collect(),
    }
}

// ─── Préparation ──────────────────────────────────────────────────────

fn prepare(points: &[RawPoint], params: &AltimetrieParams) -> Result<PreparedTrace, String> {
    if points.len() < 2 {
        return Err("Profil altimétrique indisponible : moins de 2 points avec altitude.".to_string());
    }
    if points.iter().any(|point| {
        !point.lat.is_finite() || !point.lon.is_finite() || !point.ele.is_finite()
    }) {
        return Err("Coordonnées ou altitudes invalides dans la trace.".to_string());
    }
    if !params.pas_echantillonnage.is_finite() || params.pas_echantillonnage <= 0.0 {
        return Err("Pas de rééchantillonnage invalide.".to_string());
    }

    let dist_raw = cumulative_distances(points);
    let total_dist_raw = *dist_raw.last().unwrap_or(&0.0);
    if !total_dist_raw.is_finite() || total_dist_raw <= 0.0 {
        return Err("Trace de longueur nulle : analyse altimétrique impossible.".to_string());
    }

    let ele_orig: Vec<f64> = points.iter().map(|point| point.ele).collect();
    let (ele_corrected, _) = plateau_correct(
        &ele_orig,
        params.correction_plateaux,
        PLATEAU_TOLERANCE_M,
    );

    let resampled = resample_uniform(&dist_raw, &ele_corrected, params.pas_echantillonnage);
    if resampled.dist.len() < 2 {
        return Err("Trace trop courte pour l'analyse altimétrique.".to_string());
    }

    let win_pts = despike_window_m_to_pts(params.fenetre_anti_pic, params.pas_echantillonnage);
    let (ele_raw, _) = despike_altitude(&resampled.ele, win_pts, params.seuil_anti_pic);
    let total_dist_sampled = *resampled.dist.last().unwrap_or(&0.0);

    Ok(PreparedTrace {
        points: points.to_vec(),
        dist_raw,
        dist: resampled.dist,
        ele_raw,
        total_dist_raw,
        total_dist_sampled,
        trailing_gap_m: (total_dist_raw - total_dist_sampled).max(0.0),
        step_m: params.pas_echantillonnage,
    })
}

/// Distances cumulées 2D (Haversine), dans l'ordre du fichier.
fn cumulative_distances(points: &[RawPoint]) -> Vec<f64> {
    let mut dist = Vec::with_capacity(points.len());
    dist.push(0.0);
    let mut total = 0.0;
    for window in points.windows(2) {
        total += haversine(window[0].lat, window[0].lon, window[1].lat, window[1].lon);
        dist.push(total);
    }
    dist
}

fn haversine(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let phi1 = lat1.to_radians();
    let phi2 = lat2.to_radians();
    let dphi = (lat2 - lat1).to_radians();
    let dlambda = (lon2 - lon1).to_radians();
    let a = ((dphi / 2.0).sin().powi(2)
        + phi1.cos() * phi2.cos() * (dlambda / 2.0).sin().powi(2))
    .clamp(0.0, 1.0);
    2.0 * EARTH_RADIUS_M * a.sqrt().atan2((1.0 - a).sqrt())
}

// ─── Prétraitement (portage du prototype) ────────────────────────────

/// Correction des séries plates : interpolation entre les points qui les
/// encadrent. `min_run < 2` désactive la correction.
pub fn plateau_correct(ele: &[f64], min_run: i32, tolerance: f64) -> (Vec<f64>, usize) {
    let n = ele.len();
    if n < 3 || min_run < 2 {
        return (ele.to_vec(), 0);
    }
    let min_run = min_run as usize;
    let mut out = ele.to_vec();
    let mut count = 0;
    let mut i = 0;
    while i < n {
        let mut j = i + 1;
        while j < n && (ele[j] - ele[i]).abs() <= tolerance {
            j += 1;
        }
        let run_len = j - i;
        if run_len >= min_run && i > 0 && j < n {
            let e_start = ele[i - 1];
            let e_end = ele[j];
            let span = (j - (i - 1)) as f64;
            for k in i..j {
                let t = (k - (i - 1)) as f64 / span;
                out[k] = e_start + (e_end - e_start) * t;
            }
            count += 1;
        }
        i = j;
    }
    (out, count)
}

/// Rééchantillonnage par moyenne. La fenêtre de moyennage vaut
/// `min(max(pas, 75), 3 × pas)`, comme le prototype.
fn resample_uniform(dist_raw: &[f64], ele_raw: &[f64], step_m: f64) -> Resampled {
    if dist_raw.len() < 2
        || ele_raw.len() != dist_raw.len()
        || !step_m.is_finite()
        || step_m <= 0.0
    {
        return Resampled {
            dist: Vec::new(),
            ele: Vec::new(),
        };
    }

    let total_m = *dist_raw.last().unwrap_or(&0.0);
    let m = (total_m / step_m).floor() as usize + 1;
    let mut dist = Vec::with_capacity(m);
    let mut ele = Vec::with_capacity(m);
    let avg_win = step_m.max(RESAMPLE_AVG_MIN_M).min(3.0 * step_m);
    let half_w = avg_win / 2.0;
    let mut start_ptr = 0usize;

    for k in 0..m {
        let d_center = k as f64 * step_m;
        dist.push(d_center);
        let lo = d_center - half_w;
        let hi = d_center + half_w;

        while start_ptr < dist_raw.len() - 1 && dist_raw[start_ptr] < lo {
            start_ptr += 1;
        }

        let mut sum_ele = 0.0;
        let mut count = 0usize;
        for i in start_ptr..dist_raw.len() {
            if dist_raw[i] >= hi {
                break;
            }
            if dist_raw[i] >= lo {
                sum_ele += ele_raw[i];
                count += 1;
            }
        }

        if count > 0 {
            ele.push(sum_ele / count as f64);
        } else {
            let mut jj = 0usize;
            while jj + 2 < dist_raw.len() && dist_raw[jj + 1] < d_center {
                jj += 1;
            }
            let d0 = dist_raw[jj];
            let d1 = dist_raw[jj + 1];
            if d1 - d0 < 1e-6 {
                ele.push(ele_raw[jj]);
            } else {
                let t = (d_center - d0) / (d1 - d0);
                ele.push(ele_raw[jj] + (ele_raw[jj + 1] - ele_raw[jj]) * t);
            }
        }
    }

    Resampled { dist, ele }
}

fn despike_window_m_to_pts(window_m: f64, step_m: f64) -> usize {
    let rounded = (window_m / step_m).round() as i64;
    (rounded | 1).max(3) as usize
}

/// Anti-pic de Hampel : remplace un point trop éloigné de la médiane locale.
/// Les bords du buffer sont répétés, comme dans le prototype.
pub fn despike_altitude(ele: &[f64], win_pts: usize, threshold_m: f64) -> (Vec<f64>, usize) {
    let n = ele.len();
    if n < 3 || win_pts < 3 || !threshold_m.is_finite() || threshold_m <= 0.0 {
        return (ele.to_vec(), 0);
    }
    let win = if win_pts % 2 == 0 { win_pts + 1 } else { win_pts };
    let half = win / 2;
    let mut out = ele.to_vec();
    let mut buf = vec![0.0; 2 * half + 1];
    let mut count = 0;

    for i in 0..n {
        for (k, j) in (-(half as i64)..=(half as i64)).enumerate() {
            let idx = (i as i64 + j).clamp(0, n as i64 - 1) as usize;
            buf[k] = ele[idx];
        }
        buf.sort_by(|a, b| a.total_cmp(b));
        let median = buf[half];
        if (ele[i] - median).abs() > threshold_m {
            out[i] = median;
            count += 1;
        }
    }

    (out, count)
}

// ─── Lissage ──────────────────────────────────────────────────────────

fn moving_average(arr: &[f64], win: usize) -> Vec<f64> {
    let n = arr.len();
    if win < 2 || n < 2 {
        return arr.to_vec();
    }
    let half = win / 2;
    let mut prefix = vec![0.0; n + 1];
    for (i, value) in arr.iter().enumerate() {
        prefix[i + 1] = prefix[i] + value;
    }
    let mut out = vec![0.0; n];
    for i in 0..n {
        let lo = i.saturating_sub(half);
        let hi = (i + half).min(n - 1);
        out[i] = (prefix[hi + 1] - prefix[lo]) / (hi - lo + 1) as f64;
    }
    out
}

/// Savitzky-Golay quadratique. Le paramètre d'ordre du prototype est ignoré :
/// l'implémentation est toujours d'ordre 2.
fn savitzky_golay(arr: &[f64], win: usize) -> Vec<f64> {
    let n = arr.len();
    if win < 5 || n < 5 {
        return arr.to_vec();
    }
    let win = if win % 2 == 0 { win + 1 } else { win };
    let half = win / 2;
    let m = half as f64;
    let coefs: Vec<f64> = (-(half as i64)..=(half as i64))
        .map(|k| {
            let k = k as f64;
            3.0 * m * m + 3.0 * m - 1.0 - 5.0 * k * k
        })
        .collect();
    let mut out = vec![0.0; n];

    for i in 0..n {
        let mut sum = 0.0;
        let mut wsum = 0.0;
        for j in -(half as i64)..=(half as i64) {
            let idx = i as i64 + j;
            if idx < 0 || idx >= n as i64 {
                continue;
            }
            let c = coefs[(j + half as i64) as usize];
            sum += c * arr[idx as usize];
            wsum += c;
        }
        out[i] = if wsum > 0.0 { sum / wsum } else { arr[i] };
    }
    out
}

fn gaussian_filter(arr: &[f64], sigma: f64) -> Vec<f64> {
    let n = arr.len();
    if !sigma.is_finite() || sigma <= 0.0 || n < 2 {
        return arr.to_vec();
    }
    let half = (3.0 * sigma).ceil() as i64;
    let mut kernel = vec![0.0; (2 * half + 1) as usize];
    let mut ksum = 0.0;
    for offset in -half..=half {
        let offset_f = offset as f64;
        let value = (-(offset_f * offset_f) / (2.0 * sigma * sigma)).exp();
        kernel[(offset + half) as usize] = value;
        ksum += value;
    }
    for value in &mut kernel {
        *value /= ksum;
    }

    let mut out = vec![0.0; n];
    for i in 0..n {
        let mut sum = 0.0;
        let mut wsum = 0.0;
        for j in -half..=half {
            let idx = i as i64 + j;
            if idx < 0 || idx >= n as i64 {
                continue;
            }
            let w = kernel[(j + half) as usize];
            sum += w * arr[idx as usize];
            wsum += w;
        }
        out[i] = if wsum > 0.0 { sum / wsum } else { arr[i] };
    }
    out
}

fn median_filter(arr: &[f64], win: usize) -> Vec<f64> {
    let n = arr.len();
    if win < 3 || n < 3 {
        return arr.to_vec();
    }
    let win = if win % 2 == 0 { win + 1 } else { win };
    let half = win / 2;
    let mut out = vec![0.0; n];
    let mut buf = vec![0.0; 2 * half + 1];

    for i in 0..n {
        for (k, j) in (-(half as i64)..=(half as i64)).enumerate() {
            let idx = (i as i64 + j).clamp(0, n as i64 - 1) as usize;
            buf[k] = arr[idx];
        }
        buf.sort_by(|a, b| a.total_cmp(b));
        out[i] = buf[half];
    }
    out
}

/// Lissage final : Savitzky-Golay, gaussien optionnel, correction linéaire
/// ancrée aux indices fournis (en pratique, les extrémités de la trace), puis
/// double passage optionnel.
pub fn anchored_moving_average(
    raw: &[f64],
    anchor_idxs: &[usize],
    step_m: f64,
    fenetre_altitude_m: f64,
    sigma_m: f64,
    double_sg: bool,
) -> Vec<f64> {
    let n = raw.len();
    if n < 2 {
        return raw.to_vec();
    }
    if !step_m.is_finite() || step_m <= 0.0 {
        return raw.to_vec();
    }

    let win_sg = (((fenetre_altitude_m / step_m).round() as i64) | 1).max(5) as usize;
    let mut base = savitzky_golay(raw, win_sg);
    if sigma_m > 0.0 {
        base = gaussian_filter(&base, sigma_m / step_m);
    }

    let mut set = BTreeSet::new();
    set.insert(0);
    set.insert(n - 1);
    for &idx in anchor_idxs {
        if idx > 0 && idx < n - 1 {
            set.insert(idx);
        }
    }
    let anchors: Vec<usize> = set.into_iter().collect();

    let mut result = base.clone();
    for pair in anchors.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if b - a < 2 {
            continue;
        }
        let corr_a = raw[a] - base[a];
        let corr_b = raw[b] - base[b];
        let span = (b - a) as f64;
        for i in a..=b {
            let t = (i - a) as f64 / span;
            result[i] = base[i] + corr_a * (1.0 - t) + corr_b * t;
        }
    }

    if double_sg {
        let win_double = (((fenetre_altitude_m * 2.0 / step_m).round() as i64) | 1).max(31) as usize;
        let mut final_out = savitzky_golay(&result, win_double);
        for &anchor in &anchors {
            final_out[anchor] = result[anchor];
        }
        final_out
    } else {
        result
    }
}

// ─── Pentes et catégories ─────────────────────────────────────────────

fn compute_slope(ele: &[f64], dist: &[f64]) -> Vec<f64> {
    let n = ele.len();
    let mut slope = vec![0.0; n];
    if n < 2 {
        return slope;
    }
    for i in 0..n {
        let i1 = i.saturating_sub(1);
        let i2 = (i + 1).min(n - 1);
        let d = dist[i2] - dist[i1];
        if d > 0.05 {
            slope[i] = (ele[i2] - ele[i1]) / d * 100.0;
        }
    }
    slope
}

fn compute_slope_window(ele: &[f64], dist: &[f64], win_m: f64) -> Vec<f64> {
    let n = ele.len();
    let mut slope = vec![0.0; n];
    if n < 2 {
        return slope;
    }
    for i in 0..n {
        let mut lo = i;
        while lo > 0 && dist[i] - dist[lo - 1] <= win_m {
            lo -= 1;
        }
        let mut hi = i;
        while hi < n - 1 && dist[hi + 1] - dist[i] <= win_m {
            hi += 1;
        }
        let d_dist = dist[hi] - dist[lo];
        if d_dist > 0.5 {
            slope[i] = (ele[hi] - ele[lo]) / d_dist * 100.0;
        }
    }
    slope
}

fn clip_slope(slope: &[f64], max_pct: f64) -> Vec<f64> {
    slope
        .iter()
        .map(|&value| value.clamp(-max_pct, max_pct))
        .collect()
}

fn categorize(slope: f64) -> &'static str {
    if slope.is_nan() {
        return "pl";
    }
    if slope < -7.0 {
        "dh3"
    } else if slope < -3.0 {
        "dh2"
    } else if slope < -1.0 {
        "dh1"
    } else if slope < 1.0 {
        "pl"
    } else if slope < 3.0 {
        "up1"
    } else if slope < 7.0 {
        "up2"
    } else if slope < 10.0 {
        "up3"
    } else {
        "up4"
    }
}

fn min_slope_for_length(length: f64) -> f64 {
    if length >= 1_000.0 {
        3.0
    } else if length >= 750.0 {
        4.0
    } else if length >= 500.0 {
        5.0
    } else if length >= 400.0 {
        6.0
    } else if length >= 300.0 {
        7.0
    } else {
        f64::INFINITY
    }
}

fn find_idx_at_dist(dist: &[f64], target_m: f64) -> usize {
    let mut lo = 0usize;
    let mut hi = dist.len().saturating_sub(1);
    while lo < hi {
        let mid = (lo + hi) >> 1;
        if dist[mid] < target_m {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

fn argmin_range(arr: &[f64], lo: usize, hi: usize) -> usize {
    let mut best = lo;
    let mut value = arr[lo];
    for (i, &candidate) in arr.iter().enumerate().take(hi + 1).skip(lo + 1) {
        if candidate < value {
            value = candidate;
            best = i;
        }
    }
    best
}

fn argmax_range(arr: &[f64], lo: usize, hi: usize) -> usize {
    let mut best = lo;
    let mut value = arr[lo];
    for (i, &candidate) in arr.iter().enumerate().take(hi + 1).skip(lo + 1) {
        if candidate > value {
            value = candidate;
            best = i;
        }
    }
    best
}

fn d_plus_range(ele: &[f64], start_idx: usize, end_idx: usize) -> f64 {
    if end_idx <= start_idx {
        return 0.0;
    }
    let mut total = 0.0;
    for k in start_idx + 1..=end_idx {
        let delta = ele[k] - ele[k - 1];
        if delta > 0.0 {
            total += delta;
        }
    }
    total
}

// ─── Segmentation par catégorie ───────────────────────────────────────

fn merge_short_segments(mut raw: Vec<RawSegment>, dist: &[f64], min_len: f64) -> Vec<RawSegment> {
    let mut changed = true;
    while changed && raw.len() > 1 {
        changed = false;
        let mut i = 0;
        while i < raw.len() {
            let segment = raw[i].clone();
            let length = dist[segment.end_idx] - dist[segment.start_idx];
            if length >= min_len {
                i += 1;
                continue;
            }
            let left = if i > 0 {
                dist[raw[i - 1].end_idx] - dist[raw[i - 1].start_idx]
            } else {
                -1.0
            };
            let right = if i < raw.len() - 1 {
                dist[raw[i + 1].end_idx] - dist[raw[i + 1].start_idx]
            } else {
                -1.0
            };
            if right >= left {
                raw[i + 1].start_idx = segment.start_idx;
            } else {
                raw[i - 1].end_idx = segment.end_idx;
            }
            raw.remove(i);
            changed = true;
            break;
        }
    }

    if raw.len() > 1 {
        let mut merged: Vec<RawSegment> = Vec::with_capacity(raw.len());
        merged.push(raw[0].clone());
        for segment in raw.into_iter().skip(1) {
            let last = merged.last_mut().expect("au moins un segment");
            if last.key == segment.key {
                last.end_idx = segment.end_idx;
            } else {
                merged.push(segment);
            }
        }
        merged
    } else {
        raw
    }
}

fn segment_by_category(
    slope_for_cat: &[f64],
    dist: &[f64],
    ele_metric: &[f64],
    min_len: f64,
    total_dist_raw: f64,
) -> Vec<AltimetrieSegment> {
    let categories: Vec<&'static str> = slope_for_cat.iter().map(|&s| categorize(s)).collect();
    let mut raw = Vec::new();
    let mut start = 0usize;
    for i in 1..=categories.len() {
        if i == categories.len() || categories[i] != categories[start] {
            let end_idx = if i == categories.len() { i - 1 } else { i };
            raw.push(RawSegment {
                start_idx: start,
                end_idx,
                key: categories[start],
            });
            start = i;
        }
    }

    merge_short_segments(raw, dist, min_len.max(0.0))
        .into_iter()
        .enumerate()
        .map(|(index, segment)| {
            let start_dist = dist[segment.start_idx];
            let end_dist = dist[segment.end_idx];
            let length = end_dist - start_dist;
            let d_plus = d_plus_range(ele_metric, segment.start_idx, segment.end_idx);
            let mut weighted = 0.0;
            for k in segment.start_idx + 1..=segment.end_idx {
                weighted += slope_for_cat[k] * (dist[k] - dist[k - 1]);
            }
            let avg_slope = if length > 0.0 { weighted / length } else { 0.0 };

            AltimetrieSegment {
                index,
                start_km: round_to(start_dist / 1000.0, 4),
                end_km: round_to(end_dist / 1000.0, 4),
                start_progress: round_to(start_dist / total_dist_raw, 5),
                end_progress: round_to(end_dist / total_dist_raw, 5),
                category_id: segment.key.to_string(),
                avg_slope_pct: round_to(avg_slope, 2),
                d_plus_m: d_plus.round(),
            }
        })
        .collect()
}

// ─── Détection automatique ────────────────────────────────────────────

fn detect_events(slope: &[f64], threshold: f64) -> Vec<Event> {
    let mut events = Vec::new();
    let mut in_climb = false;
    for (idx, &value) in slope.iter().enumerate() {
        if !in_climb && value > threshold {
            events.push(Event {
                idx,
                kind: EventKind::Start,
            });
            in_climb = true;
        } else if in_climb && value < -threshold {
            events.push(Event {
                idx,
                kind: EventKind::Summit,
            });
            in_climb = false;
        }
    }
    if in_climb {
        events.push(Event {
            idx: slope.len() - 1,
            kind: EventKind::Summit,
        });
    }
    events
}

fn build_climb_candidates(events: &[Event], n: usize) -> Vec<Candidate> {
    let mut segments: Vec<(usize, usize, bool)> = Vec::new();
    let mut prev = 0usize;
    let mut current_is_climb = events
        .first()
        .map(|event| event.kind == EventKind::Summit)
        .unwrap_or(false);

    for event in events {
        if event.idx > prev {
            segments.push((prev, event.idx, current_is_climb));
        }
        current_is_climb = event.kind == EventKind::Start;
        prev = event.idx;
    }
    if prev < n - 1 {
        segments.push((prev, n - 1, current_is_climb));
    }

    segments
        .into_iter()
        .filter(|(_, _, is_climb)| *is_climb)
        .map(|(start_idx, end_idx, _)| Candidate {
            start_idx,
            end_idx,
        })
        .collect()
}

fn find_best_sub_climb(
    dist: &[f64],
    ele_metric: &[f64],
    start_idx: usize,
    end_idx: usize,
) -> Option<SubClimb> {
    if end_idx <= start_idx {
        return None;
    }
    let n = end_idx - start_idx + 1;
    if n < 2 {
        return None;
    }

    let mut cum = vec![0.0; n];
    for k in 1..n {
        let delta = ele_metric[start_idx + k] - ele_metric[start_idx + k - 1];
        cum[k] = cum[k - 1] + delta.max(0.0);
    }

    let step = if n > 800 {
        ((n as f64) / 800.0).ceil() as usize
    } else {
        1
    }
    .max(1);

    let mut best: Option<SubClimb> = None;
    for si in (0..n - 1).step_by(step) {
        for ei in (si + 1..n).step_by(step) {
            let length = dist[start_idx + ei] - dist[start_idx + si];
            if length < 300.0 {
                continue;
            }
            if best.as_ref().is_some_and(|current| length <= current.length) {
                continue;
            }
            let d_plus = cum[ei] - cum[si];
            let slope = d_plus / length * 100.0;
            if slope >= min_slope_for_length(length) {
                best = Some(SubClimb {
                    start_idx: start_idx + si,
                    end_idx: start_idx + ei,
                    length,
                });
            }
        }
    }
    best
}

fn compute_auto(prepared: &PreparedTrace, params: &AltimetrieParams) -> Vec<AutoClimb> {
    let n = prepared.ele_raw.len();
    let step = prepared.step_m;

    let ele_median = if params.fenetre_mediane > 0.0 {
        let win = (((params.fenetre_mediane / step).round() as i64) | 1).max(3) as usize;
        median_filter(&prepared.ele_raw, win)
    } else {
        prepared.ele_raw.clone()
    };

    let win_sg = (((params.fenetre_altitude / step).round() as i64) | 1).max(5) as usize;
    let ele_base = savitzky_golay(&ele_median, win_sg);

    let win_slope = ((params.fenetre_pente / step).round() as i64).max(1) as usize;
    let slope_base = clip_slope(
        &moving_average(&compute_slope(&ele_base, &prepared.dist), win_slope),
        SLOPE_DISPLAY_MAX_PCT,
    );

    let candidates = build_climb_candidates(
        &detect_events(&slope_base, params.seuil_montee),
        n,
    );
    let candidates_with_sub: Vec<SubClimb> = candidates
        .iter()
        .filter_map(|candidate| {
            find_best_sub_climb(
                &prepared.dist,
                &ele_base,
                candidate.start_idx,
                candidate.end_idx,
            )
        })
        .collect();

    let mut refined: Vec<RefinedClimb> = Vec::new();
    let mut last_end: i64 = -1;

    for candidate in &candidates_with_sub {
        let effective_refine = (params.recherche_extrema * candidate.length / 100.0)
            .clamp(REFINE_MIN_M, REFINE_MAX_M);

        let hi_target = (prepared.dist[candidate.end_idx] + effective_refine)
            .min(prepared.dist[n - 1]);
        let hi_idx = find_idx_at_dist(&prepared.dist, hi_target);
        let new_end = if hi_idx > candidate.end_idx {
            argmax_range(&ele_base, candidate.end_idx, hi_idx)
        } else {
            candidate.end_idx
        };
        if new_end <= candidate.start_idx {
            continue;
        }
        if new_end as i64 <= last_end {
            continue;
        }

        let natural_low = (last_end + 1).max(0) as usize;
        let safety_low = find_idx_at_dist(
            &prepared.dist,
            (prepared.dist[candidate.start_idx] - effective_refine).max(0.0),
        );
        let lo_idx = natural_low.max(safety_low);
        let new_start = if lo_idx < candidate.start_idx {
            argmin_range(&ele_base, lo_idx, candidate.start_idx)
        } else {
            candidate.start_idx
        };
        if new_start >= new_end {
            continue;
        }

        if !refined.is_empty() && params.ecart_fusion > 0.0 {
            let gap_m = prepared.dist[new_start] - prepared.dist[refined.last().unwrap().end_idx];
            if gap_m < params.ecart_fusion {
                let previous = refined.last_mut().expect("au moins une montée");
                if previous.merged_parts.is_none() {
                    previous.merged_parts = Some(vec![BoundsKm {
                        start_km: prepared.dist[previous.start_idx] / 1000.0,
                        end_km: prepared.dist[previous.end_idx] / 1000.0,
                    }]);
                }

                let combined_end = argmax_range(&ele_base, previous.end_idx, new_end);
                let mid_valley = argmin_range(&ele_base, previous.end_idx, new_start);
                let mut new_previous_start = previous.start_idx;
                if ele_base[mid_valley] < ele_base[previous.start_idx] {
                    new_previous_start = mid_valley;
                }

                previous
                    .merged_parts
                    .as_mut()
                    .expect("initialisé ci-dessus")
                    .push(BoundsKm {
                        start_km: prepared.dist[new_start] / 1000.0,
                        end_km: prepared.dist[new_end] / 1000.0,
                    });
                previous.start_idx = new_previous_start;
                previous.end_idx = combined_end;
                last_end = combined_end as i64;
                continue;
            }
        }

        refined.push(RefinedClimb {
            start_idx: new_start,
            end_idx: new_end,
            merged_parts: None,
        });
        last_end = new_end as i64;
    }

    // Seules les extrémités de la trace sont ancrées : forcer le profil à
    // repasser par l'altitude brute des bornes de montée recréait un pic ou
    // un creux d'un point là où le lissage avait aplati le relief.
    let ele_display = anchored_moving_average(
        &prepared.ele_raw,
        &[],
        step,
        params.fenetre_altitude,
        params.sigma_gaussien,
        params.lissage_renforce,
    );

    let mut auto_climbs = Vec::new();
    for climb in &refined {
        let mut final_start = climb.start_idx;
        let mut final_end = climb.end_idx;
        let length = prepared.dist[final_end] - prepared.dist[final_start];
        let d_plus = d_plus_range(&ele_display, final_start, final_end);
        let avg_slope = if length > 0.0 {
            d_plus / length * 100.0
        } else {
            0.0
        };
        let min_slope = min_slope_for_length(length);
        let mut fallback = false;

        if length < 300.0 || avg_slope < min_slope {
            match find_best_sub_climb(
                &prepared.dist,
                &ele_display,
                climb.start_idx,
                climb.end_idx,
            ) {
                Some(sub) => {
                    final_start = sub.start_idx;
                    final_end = sub.end_idx;
                    fallback = true;
                }
                None => continue,
            }
        }

        auto_climbs.push(AutoClimb {
            start_km: prepared.dist[final_start] / 1000.0,
            end_km: prepared.dist[final_end] / 1000.0,
            merged_parts: climb.merged_parts.clone(),
            fallback,
        });
    }

    auto_climbs
}

fn auto_to_edit(climb: &AutoClimb, id: u32) -> AltimetrieClimbEdit {
    let parts = climb.merged_parts.as_ref().map(|parts| {
        parts
            .iter()
            .map(|part| AltimetrieMergedPartEdit {
                start_km: part.start_km,
                end_km: part.end_km,
            })
            .collect::<Vec<_>>()
    });
    let is_merged = parts.as_ref().is_some_and(|parts| parts.len() >= 2);

    AltimetrieClimbEdit {
        id,
        start_km: climb.start_km,
        end_km: climb.end_km,
        is_merged,
        is_separated: false,
        group_id: None,
        merged_parts: if is_merged { parts } else { None },
        fallback: climb.fallback,
    }
}

// ─── Reconstruction après édition ─────────────────────────────────────

fn rebuild(
    prepared: &PreparedTrace,
    params: &AltimetrieParams,
    climbs: &[AltimetrieClimbEdit],
) -> Rebuilt {
    let mut sorted = climbs.to_vec();
    sorted.sort_by(|a, b| {
        a.start_km
            .partial_cmp(&b.start_km)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Contraintes du prototype : pas de chevauchement, longueur minimale de
    // 100 m après résolution.
    let mut previous_end_km = f64::NEG_INFINITY;
    for climb in &mut sorted {
        if climb.start_km < previous_end_km {
            climb.start_km = previous_end_km + 0.001;
        }
        if climb.end_km <= climb.start_km {
            climb.end_km = climb.start_km + 0.1;
        }
        previous_end_km = climb.end_km;
    }

    let mut resolved: Vec<(AltimetrieClimbEdit, usize, usize)> = Vec::new();
    for climb in &sorted {
        let start_idx = find_idx_at_dist(&prepared.dist, climb.start_km * 1000.0);
        let end_idx = find_idx_at_dist(&prepared.dist, climb.end_km * 1000.0);
        if start_idx >= end_idx {
            continue;
        }
        resolved.push((climb.clone(), start_idx, end_idx));
    }

    // Même règle que la détection : les bornes de montée ne sont pas ancrées
    // sur l'altitude brute, sous peine de recréer un pic d'un point.
    let display_ele = anchored_moving_average(
        &prepared.ele_raw,
        &[],
        prepared.step_m,
        params.fenetre_altitude,
        params.sigma_gaussien,
        params.lissage_renforce,
    );

    let win_sg_cat =
        (((params.fenetre_altitude / prepared.step_m).round() as i64) | 1).max(5) as usize;
    let ele_base_cat = savitzky_golay(&prepared.ele_raw, win_sg_cat);
    let slope_for_cat =
        compute_slope_window(&ele_base_cat, &prepared.dist, params.fenetre_calcul);

    let segments = segment_by_category(
        &slope_for_cat,
        &prepared.dist,
        &display_ele,
        params.longueur_min,
        prepared.total_dist_raw,
    );

    let mut out_climbs = Vec::with_capacity(resolved.len());
    for (climb, start_idx, end_idx) in resolved {
        let length = prepared.dist[end_idx] - prepared.dist[start_idx];
        let d_plus = d_plus_range(&display_ele, start_idx, end_idx);
        let avg_slope = if length > 0.0 {
            d_plus / length * 100.0
        } else {
            0.0
        };

        let merged_parts = climb.merged_parts.as_ref().map(|parts| {
            let mut out = Vec::new();
            for part in parts {
                let part_start = find_idx_at_dist(&prepared.dist, part.start_km * 1000.0);
                let part_end = find_idx_at_dist(&prepared.dist, part.end_km * 1000.0);
                if part_start >= part_end {
                    continue;
                }
                let part_length = prepared.dist[part_end] - prepared.dist[part_start];
                let part_d_plus = d_plus_range(&display_ele, part_start, part_end);
                let part_avg = if part_length > 0.0 {
                    part_d_plus / part_length * 100.0
                } else {
                    0.0
                };
                out.push(AltimetrieMergedPart {
                    part_index: out.len(),
                    start_km: round_to(prepared.dist[part_start] / 1000.0, 4),
                    end_km: round_to(prepared.dist[part_end] / 1000.0, 4),
                    start_progress: round_to(
                        prepared.dist[part_start] / prepared.total_dist_raw,
                        5,
                    ),
                    end_progress: round_to(
                        prepared.dist[part_end] / prepared.total_dist_raw,
                        5,
                    ),
                    d_plus_m: part_d_plus.round(),
                    avg_slope_pct: round_to(part_avg, 2),
                    alt_start_m: round_to(display_ele[part_start], 1),
                    alt_end_m: round_to(display_ele[part_end], 1),
                });
            }
            out
        });

        let is_merged = climb.is_merged
            && merged_parts
                .as_ref()
                .is_some_and(|parts| parts.len() >= 2);
        let merged_parts = if is_merged { merged_parts } else { None };
        let is_separated =
            !is_merged && climb.is_separated && climb.group_id.is_some();
        let group_id = if is_separated { climb.group_id } else { None };
        let merged_count = merged_parts
            .as_ref()
            .map_or(0, |parts| (parts.len() - 1) as u32);

        out_climbs.push(AltimetrieClimb {
            id: climb.id,
            index: 0,
            start_km: round_to(prepared.dist[start_idx] / 1000.0, 4),
            end_km: round_to(prepared.dist[end_idx] / 1000.0, 4),
            start_progress: round_to(prepared.dist[start_idx] / prepared.total_dist_raw, 5),
            end_progress: round_to(prepared.dist[end_idx] / prepared.total_dist_raw, 5),
            d_plus_m: d_plus.round(),
            avg_slope_pct: round_to(avg_slope, 2),
            alt_start_m: round_to(display_ele[start_idx], 1),
            alt_end_m: round_to(display_ele[end_idx], 1),
            merged_count,
            fallback: climb.fallback,
            is_merged,
            is_separated,
            group_id,
            merged_parts,
        });
    }

    for (index, climb) in out_climbs.iter_mut().enumerate() {
        climb.index = index;
    }

    Rebuilt {
        display_ele,
        segments,
        climbs: out_climbs,
    }
}

// ─── Profil et coordonnées ────────────────────────────────────────────

fn build_profile(prepared: &PreparedTrace, display_ele: &[f64]) -> Vec<AltimetrieProfilePoint> {
    prepared
        .dist
        .iter()
        .zip(display_ele.iter())
        .map(|(&distance, &ele)| {
            let (lat, lon) = interpolate_lat_lon(&prepared.points, &prepared.dist_raw, distance);
            AltimetrieProfilePoint {
                d: round_to(distance, 1),
                ele: round_to(ele, 1),
                lat: round_to(lat, 6),
                lon: round_to(lon, 6),
            }
        })
        .collect()
}

fn interpolate_lat_lon(points: &[RawPoint], dist_raw: &[f64], target_m: f64) -> (f64, f64) {
    if points.is_empty() || dist_raw.is_empty() {
        return (0.0, 0.0);
    }
    let last = points.len() - 1;
    if target_m <= dist_raw[0] {
        return (points[0].lat, points[0].lon);
    }
    if target_m >= dist_raw[last] {
        return (points[last].lat, points[last].lon);
    }

    let i2 = find_idx_at_dist(dist_raw, target_m).min(last);
    let i1 = i2.saturating_sub(1);
    let d1 = dist_raw[i1];
    let d2 = dist_raw[i2];
    let t = if d2 > d1 {
        (target_m - d1) / (d2 - d1)
    } else {
        0.0
    };
    (
        points[i1].lat + (points[i2].lat - points[i1].lat) * t,
        points[i1].lon + (points[i2].lon - points[i1].lon) * t,
    )
}

// ─── Utilitaires ──────────────────────────────────────────────────────

fn round_to(value: f64, decimals: i32) -> f64 {
    if !value.is_finite() {
        return value;
    }
    let factor = 10f64.powi(decimals);
    (value * factor).round() / factor
}
