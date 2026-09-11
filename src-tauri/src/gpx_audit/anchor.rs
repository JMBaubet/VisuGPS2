//! Calcul dérivé — ancres d'accès d'une boucle RP (ANALYSE §13).
//!
//! Portage fidèle de `rpAnchorIndices` du HTML de référence
//! (`docs/audit/reference/verifgpx-V3.0.html`).
//!
//! Calcul **dérivé** et **lazy** : consommé par l'application hôte (bornes par
//! défaut du routage, élargissement D2 des emprises), recalculé sur la trace
//! courante à chaque usage, jamais stocké dans le finding — il suit ainsi les
//! éditions sans aucun état à maintenir.
//!
//! Toutes les grandeurs sont en **indices de référence** (la géométrie reçue en
//! `px`/`py` est celle de la trace consolidée, pas celle des échantillons `R`).

use super::rp::{
    RP_ANCHOR_DELTA_ABS, RP_ANCHOR_DELTA_K, RP_ANCHOR_EXT_M, RP_ANCHOR_HARD_M,
    RP_ANCHOR_MAX_SAMPLES, RP_ANCHOR_PERSIST, RP_ANCHOR_SEARCH_BASE,
};

/// Bornes de plausibilité du rayon ajusté (m) — seuils internes de l'ANALYSE
/// §3.2 : hors de cette plage, l'ajustement n'est pas adopté.
const RP_FIT_R_MIN: f64 = 3.0;
const RP_FIT_R_MAX: f64 = 1000.0;

/// Déterminant minimal de la matrice normale de Kåsa (stabilité numérique).
const RP_FIT_DET_MIN: f64 = 1e-6;

/// Résultat du calcul d'ancres (ANALYSE §13).
///
/// `up`/`dn` sont des **indices de référence** (ou `None` : repli silencieux de
/// l'appelant sur `junc − 1` / `ce + 1`). `cx`/`cy`/`r` décrivent le cercle
/// ajusté (centre en coordonnées métriques, rayon médian en mètres).
#[derive(Debug, Clone)]
pub struct AnchorResult {
    pub up: Option<usize>, // ancre amont (index de référence) ou None
    pub dn: Option<usize>, // ancre aval
    pub cx: Option<f64>,   // centre ajusté (métrique) ou None
    pub cy: Option<f64>,
    pub r: Option<f64>, // rayon médian (m)
}

/// Ajustement de cercle par la méthode de Kåsa (moindres carrés, ANALYSE §13.2).
///
/// Résout `[[Suu, Suv, Su], [Suv, Svv, Sv], [Su, Sv, n]] · (a, b, c) = (Suz, Svz, Sz)`
/// par la règle de Cramer sur les coordonnées centrées, puis retourne le centre
/// ajusté et `rFit = √(c + a²/4 + b²/4)`.
///
/// `None` si l'ajustement est inexploitable : moins de 3 points, système
/// singulier (déterminant ≤ 1e-6) ou rayon hors de `[3, 1000]` m — l'appelant
/// retombe alors sur le centroïde (correct seulement sur un arc ≈ 360°).
pub(super) fn fit_circle_kasa(samples: &[(f64, f64)]) -> Option<(f64, f64, f64)> {
    if samples.len() < 3 {
        return None;
    }

    let nn = samples.len() as f64;
    let mut cx0 = 0.0;
    let mut cy0 = 0.0;
    for &(x, y) in samples {
        cx0 += x;
        cy0 += y;
    }
    cx0 /= nn;
    cy0 /= nn;

    let (mut suu, mut suv, mut svv, mut su, mut sv) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let (mut suz, mut svz, mut sz) = (0.0, 0.0, 0.0);
    for &(x, y) in samples {
        let u = x - cx0;
        let v = y - cy0;
        let z = u * u + v * v;
        suu += u * u;
        suv += u * v;
        svv += v * v;
        su += u;
        sv += v;
        suz += u * z;
        svz += v * z;
        sz += z;
    }

    let det = suu * (svv * nn - sv * sv) - suv * (suv * nn - su * sv) + su * (suv * sv - svv * su);
    if !det.is_finite() || det.abs() <= RP_FIT_DET_MIN {
        return None;
    }

    let da = suz * (svv * nn - sv * sv) - suv * (svz * nn - sv * sz) + su * (svz * sv - svv * sz);
    let db = suu * (svz * nn - sv * sz) - suz * (suv * nn - su * sv) + su * (suv * sz - svz * su);
    let dc = suu * (svv * sz - svz * sv) - suv * (suv * sz - svz * su) + suz * (suv * sv - svv * su);
    let a = da / det;
    let b = db / det;
    let c = dc / det;

    let r_fit = (c + a * a / 4.0 + b * b / 4.0).max(0.0).sqrt();
    if !r_fit.is_finite() || !(RP_FIT_R_MIN..=RP_FIT_R_MAX).contains(&r_fit) {
        return None;
    }

    Some((cx0 + a / 2.0, cy0 + b / 2.0, r_fit))
}

/// Test d'appartenance à la zone RP (ANALYSE §13.2).
///
/// Combine deux critères :
/// - **disque élargi** : `dist(i, centre) ≤ r + δ` avec `δ = max(12 ; 0,4·r)`
///   (le terme absolu absorbe le bruit GPS et la demi-chaussée — il ne croît pas
///   avec `r` ; le terme relatif absorbe l'imperfection de forme) ;
/// - **superposition au cœur** : `dist(i, échantillon du cœur) ≤ close_thr` —
///   couvre anneau, épingle et branche commune, insensible à la forme du cœur
///   (là où Kåsa est peu fiable).
pub(super) fn in_rp_zone(
    px: &[f64],
    py: &[f64],
    i: usize,
    cx: f64,
    cy: f64,
    r: f64,
    close_thr: f64,
    core_samples: &[(f64, f64)],
) -> bool {
    if i >= px.len() || i >= py.len() {
        return false;
    }

    let disc = r + RP_ANCHOR_DELTA_ABS.max(RP_ANCHOR_DELTA_K * r);
    let (x, y) = (px[i], py[i]);
    if (x - cx).hypot(y - cy) <= disc {
        return true;
    }

    // Sortie anticipée dès qu'un échantillon du cœur est à moins de close_thr.
    core_samples
        .iter()
        .any(|&(qx, qy)| (qx - x).hypot(qy - y) <= close_thr)
}

/// Ancres d'accès d'une boucle RP (ANALYSE §13, JS `rpAnchorIndices`).
///
/// Les points de l'anneau sont quasi **équidistants du centre** (plateau
/// radial) ; les routes d'approche et de sortie s'en écartent en `√(r² + s²)`
/// où `s` est la distance parcourue sur la route. Ce signal est indépendant de
/// la densité d'échantillonnage et invariant au diamètre (δ hybride, portées
/// fonctions de `r`).
///
/// Par côté : marche depuis la jonction (amont) ou depuis la fin du cœur
/// (aval) jusqu'à **sortie de zone confirmée** (`RP_ANCHOR_PERSIST` points
/// consécutifs), puis extension de `RP_ANCHOR_EXT_M` m interrompue par une
/// re-entrée — l'ancre marque la **frontière** de la zone anormale, pas le
/// milieu de la route franche.
///
/// Le garde de portée porte sur la distance **hors zone** depuis le dernier
/// point en zone : la traversée d'un re-parcours superposé ne consomme pas le
/// budget (leçon documentée ANALYSE §13.2).
pub fn rp_anchor_indices(
    px: &[f64],
    py: &[f64],
    junc: usize,
    ce: usize,
    close_thr: f64,
) -> AnchorResult {
    let none = AnchorResult {
        up: None,
        dn: None,
        cx: None,
        cy: None,
        r: None,
    };

    let m = px.len();
    if m == 0 || py.len() != m || junc > ce || ce >= m {
        return none;
    }

    // Échantillonnage du cœur : au plus RP_ANCHOR_MAX_SAMPLES points.
    let count = ce - junc + 1;
    let stride = count.div_ceil(RP_ANCHOR_MAX_SAMPLES).max(1);
    let mut core: Vec<(f64, f64)> = Vec::new();
    let mut i = junc;
    while i <= ce {
        core.push((px[i], py[i]));
        i += stride;
    }
    if core.is_empty() {
        return none;
    }

    // 1. Centre — centroïde, puis ajustement de Kåsa s'il est adopté.
    let mut cx = 0.0;
    let mut cy = 0.0;
    for &(x, y) in &core {
        cx += x;
        cy += y;
    }
    cx /= core.len() as f64;
    cy /= core.len() as f64;
    if let Some((kx, ky, _r_fit)) = fit_circle_kasa(&core) {
        cx = kx;
        cy = ky;
    }

    // 2. Rayon publié — MÉDIANE des distances au centre (robuste aux aberrants),
    // avec repli à 1 si elle est nulle (`ds[floor(len/2)] || 1` du JS).
    let mut ds: Vec<f64> = core
        .iter()
        .map(|&(x, y)| (x - cx).hypot(y - cy))
        .collect();
    ds.sort_by(|u, v| u.partial_cmp(v).unwrap_or(std::cmp::Ordering::Equal));
    let med = ds[ds.len() / 2];
    let r = if med > 0.0 && med.is_finite() { med } else { 1.0 };

    let search_max = RP_ANCHOR_SEARCH_BASE.max(2.5 * r);
    let in_zone = |i: usize| in_rp_zone(px, py, i, cx, cy, r, close_thr, &core);
    let seg_len = |i: usize| (px[i + 1] - px[i]).hypot(py[i + 1] - py[i]);

    // 4. Plancher puis 5. extension, par côté.
    let walk_side = |start: usize, dir: i32| -> Option<usize> {
        let mut i = start as i64;
        let mut hard = 0.0;
        let mut out_acc = 0.0;
        let mut out_start: Option<usize> = None;
        let mut out_run = 0usize;
        let mut floor: Option<usize> = None;
        let mut at_bound = false;

        loop {
            let ni = i + dir as i64;
            if ni < 0 || ni > m as i64 - 1 {
                at_bound = true;
                break;
            }
            let ni_u = ni as usize;
            let step_l = seg_len(i.min(ni) as usize);
            if hard + step_l > RP_ANCHOR_HARD_M {
                break;
            }
            hard += step_l;
            i = ni;
            if !in_zone(ni_u) {
                if out_start.is_none() {
                    out_start = Some(ni_u);
                }
                out_acc += step_l;
                if out_acc > search_max {
                    break;
                }
                out_run += 1;
                if out_run >= RP_ANCHOR_PERSIST {
                    floor = out_start;
                    break;
                }
            } else {
                out_start = None;
                out_run = 0;
                out_acc = 0.0;
            }
        }

        // C1 : borne de trace = confirmation en soi.
        if floor.is_none() && at_bound && out_start.is_some() {
            floor = out_start;
        }
        let floor = floor?;

        let mut anchor = floor;
        let mut acc_e = 0.0;
        while acc_e < RP_ANCHOR_EXT_M {
            let ni = anchor as i64 + dir as i64;
            if ni < 0 || ni > m as i64 - 1 {
                break;
            }
            let ni_u = ni as usize;
            let step_l = seg_len(anchor.min(ni_u));
            if acc_e + step_l > RP_ANCHOR_EXT_M {
                break;
            }
            if in_zone(ni_u) {
                break;
            }
            acc_e += step_l;
            anchor = ni_u;
        }

        Some(anchor)
    };

    AnchorResult {
        up: walk_side(junc, -1),
        dn: walk_side(ce, 1),
        cx: Some(cx),
        cy: Some(cy),
        r: Some(r),
    }
}
