//! Détecteur AR — aller-retours ponctuels (rebonds, aiguilles).
//!
//! Portage fidèle de `detectAR` du HTML de référence
//! (`docs/audit/reference/verifgpx-V3.0.html`), spécification normative
//! `docs/audit/spec/ANALYSE.md` §5.

use super::types::{
    AuditParams, AuditPoint, Finding, FindingContext, FindingContextIds, FindingKind,
    FindingPair, FindingPart, FindingStatus, PartRole,
};

/// Filet de sécurité des caps : un segment plus court que ce seuil (en mètres)
/// voit son cap reporté depuis un segment voisin. Inactif si σ > 0,05,
/// opérationnel si σ = 0 (trace brute).
const EPS: f64 = 0.05;

/// Écart maximal de groupage de deux sommets (en points) : au-delà, deux
/// candidats ne fusionnent jamais (spécification §5.4, `FUSE_GAP`).
const FUSE_GAP: usize = 6;

/// Phase 1 — caps des segments (azimut depuis le nord, en degrés [0, 360[).
///
/// Pour chaque segment `j` reliant `Pⱼ` à `Pⱼ₊₁` :
/// - si `hypot(dx, dy) >= EPS`, le cap vaut `(atan2(dx, dy) · 180/π + 360) % 360` ;
/// - sinon, report du cap précédent (filet EPS avant) ;
/// puis un parcours arrière reporte les caps restés indéfinis (filet arrière).
///
/// Retourne un tableau vide si le premier cap reste indéfini (trace immobile).
pub(super) fn compute_headings(px: &[f64], py: &[f64]) -> Vec<f64> {
    let m = px.len();
    // Moins de 2 points : aucun segment, donc aucun cap.
    if m < 2 {
        return Vec::new();
    }

    let mut head = vec![f64::NAN; m - 1];
    let mut prev_head = f64::NAN;

    // Parcours avant : cap direct, ou report du cap précédent sur micro-segment.
    for j in 0..m - 1 {
        let dx = px[j + 1] - px[j];
        let dy = py[j + 1] - py[j];
        if dx.hypot(dy) >= EPS {
            head[j] = (dx.atan2(dy) * 180.0 / std::f64::consts::PI + 360.0) % 360.0;
            prev_head = head[j];
        } else {
            head[j] = prev_head;
        }
    }

    // Parcours arrière : report des caps encore indéfinis depuis l'aval.
    let mut nxt = f64::NAN;
    for j in (0..m - 1).rev() {
        if !head[j].is_nan() {
            nxt = head[j];
        } else {
            head[j] = nxt;
        }
    }

    // Trace immobile : aucun cap défini nulle part.
    if head[0].is_nan() {
        return Vec::new();
    }

    head
}

/// Candidat sommet produit par `find_candidates`.
#[derive(Debug, Clone, Copy)]
pub(super) struct Candidate {
    pub(super) i: usize,    // index du sommet
    pub(super) diff: f64,   // écart de cap normalisé [0, 180] (0 = demi-tour parfait)
    pub(super) d1: f64,     // branche amont (m)
    pub(super) d2: f64,     // branche aval (m)
}

/// Phase 2 — candidats sommets (C1 retournement + C2 branches).
///
/// Pour `i` dans `[margin, m−1−margin]` :
/// - **C1** : `diff = |head[i] − head[i−1]|` réduite à `[0, 180]` ;
///   retenu si `180 − diff ≤ p-tol`.
/// - **C2** : `d1 = |P(i) − P(i−1)|`, `d2 = |P(i+1) − P(i)|` ;
///   retenu si `d1 ≤ p-seg` et `d2 ≤ p-seg`.
pub(super) fn find_candidates(
    px: &[f64],
    py: &[f64],
    headings: &[f64],
    margin: usize,
    params: &AuditParams,
) -> Vec<Candidate> {
    let m = px.len();
    let mut cands: Vec<Candidate> = Vec::new();

    // Gardes d'intégrité (détect_ar fournit toujours m ≥ 5, margin ∈ {1, 3} et
    // headings.len() == m−1) : évitent tout accès hors tableau sur entrée dégénérée.
    if m < 2 || margin < 1 || margin >= m || headings.len() < m - 1 {
        return cands;
    }

    let hi = m - 1 - margin;
    for i in margin..=hi {
        let mut diff = (headings[i] - headings[i - 1]).abs() % 360.0;
        if diff > 180.0 {
            diff = 360.0 - diff;
        }
        if 180.0 - diff > params.tol_deg {
            continue;
        }
        let d1 = (px[i] - px[i - 1]).hypot(py[i] - py[i - 1]);
        let d2 = (px[i + 1] - px[i]).hypot(py[i + 1] - py[i]);
        if d1 > params.seg_m || d2 > params.seg_m {
            continue;
        }
        cands.push(Candidate { i, diff, d1, d2 });
    }

    cands
}

/// Phase 3 — groupage des sommets (R1 voisins + R2 fuse) et sélection du
/// représentant médian de chaque groupe.
///
/// Les candidats sont triés par `i` croissant puis groupés par chaînage :
/// - **R1** : écart au dernier membre < 3 points → fusion inconditionnelle ;
/// - **R2** : écart ≤ `FUSE_GAP` et (caps jumeaux `|head[g[0].i−1] − head[c.i]| ≤ p-tol`,
///   caps opposés `|180 − même écart| ≤ p-tol`, ou sommets proches `≤ p-pair`).
///
/// Le représentant d'un groupe minimise `|c.i − milieu|` ; à égalité, le
/// retournement le plus net (`diff` maximal) ; à nouvelle égalité, le premier.
pub(super) fn group_candidates(
    candidates: &[Candidate],
    headings: &[f64],
    px: &[f64],
    py: &[f64],
    params: &AuditParams,
) -> Vec<Candidate> {
    let mut cands = candidates.to_vec();
    cands.sort_by_key(|c| c.i);

    // Sécurité : tout candidat doit indexer un cap (i ≥ 1) et un point valides
    // (détect_ar garantit i ∈ [1, m−2]). Entrée dégénérée → tri sans fusion.
    if !cands
        .iter()
        .all(|c| c.i >= 1 && c.i < headings.len() && c.i < px.len() && c.i < py.len())
    {
        return cands;
    }

    let mut groups: Vec<Vec<Candidate>> = Vec::new();
    for c in cands {
        if let Some(g) = groups.last_mut() {
            let refc = g[g.len() - 1];
            let gap = c.i - refc.i;
            if gap < 3 {
                g.push(c);
                continue;
            }
            if gap <= FUSE_GAP {
                let mut d = (headings[g[0].i - 1] - headings[c.i]).abs() % 360.0;
                if d > 180.0 {
                    d = 360.0 - d;
                }
                let far = (px[c.i] - px[refc.i]).hypot(py[c.i] - py[refc.i]);
                if d <= params.tol_deg
                    || (180.0 - d).abs() <= params.tol_deg
                    || far <= params.pair_m
                {
                    g.push(c);
                    continue;
                }
            }
        }
        groups.push(vec![c]);
    }

    groups
        .iter()
        .map(|g| {
            if g.len() == 1 {
                return g[0];
            }
            let mid = (g[0].i + g[g.len() - 1].i) as f64 / 2.0;
            let mut best = g[0];
            for &c in g {
                let dc = (c.i as f64 - mid).abs();
                let db = (best.i as f64 - mid).abs();
                if dc < db || (dc == db && c.diff > best.diff) {
                    best = c;
                }
            }
            best
        })
        .collect()
}

/// Paire miroir détectée par `find_mirror_pairs`.
#[derive(Debug, Clone, Copy)]
pub(super) struct MirrorPair {
    pub(super) a: usize, // index amont
    pub(super) b: usize, // index aval
    pub(super) d: f64,   // distance (m)
}

/// Phase 4 — paires miroirs symétriques autour d'un sommet.
///
/// Pour `k = 1..=p-maxpairs` : `a = i−k−1`, `b = i+k+1` (symétrie stricte par
/// index). Arrêt si `a < 0`, `b ≥ m`, `a ≤ prev_end`, ou `d > p-pair`
/// (arrêt à la première rejetée).
pub(super) fn find_mirror_pairs(
    peak: usize,
    px: &[f64],
    py: &[f64],
    params: &AuditParams,
    prev_end: Option<usize>,
) -> Vec<MirrorPair> {
    let m = px.len();
    let k_max = params.maxpairs as usize;
    let pair_max = params.pair_m;
    let mut pairs: Vec<MirrorPair> = Vec::new();

    for k in 1..=k_max {
        // a = peak − k − 1 : garde contre le sous-dépassement (équivaut au `a < 0` JS).
        if peak < k + 1 {
            break;
        }
        let a = peak - k - 1;
        let b = peak + k + 1;
        // `b >= m` équivaut au `b > m − 1` du JS, sans sous-dépassement si m = 0.
        if b >= m || prev_end.map_or(false, |e| a <= e) {
            break;
        }
        let d = (px[b] - px[a]).hypot(py[b] - py[a]);
        if d > pair_max {
            break;
        }
        pairs.push(MirrorPair { a, b, d });
    }

    pairs
}

/// Portage fidèle de `detectAR` (verifgpx-V3.0.html §5).
///
/// # Préconditions
/// - `points`, `px`, `py`, `ids` : tableaux parallèles de même longueur ;
/// - `points` consolidée (cf. `consolidation.rs`) — `px`/`py` projetés,
///   `ids` les identifiants stables.
///
/// # Postconditions
/// - Findings triés par `parts[0].s` croissant ;
/// - Emprises disjointes (`f[i].parts[0].s > f[i−1].parts[0].e`) ;
/// - Chaque finding a `kind = FindingKind::Ar`.
///
/// `points` n'est pas consommé par l'algorithme (le JS `detectAR` n'utilise
/// que `px`/`py`/`ids`) — conservé pour le contrat de signature.
pub fn detect_ar(
    _points: &[AuditPoint],
    px: &[f64],
    py: &[f64],
    ids: &[u32],
    params: &AuditParams,
) -> Vec<Finding> {
    let m = px.len();
    if m < 5 {
        return Vec::new();
    }

    // Marge : 3 si trace fermée (d(P0, P_last) < 100 m), sinon 1.
    let closed = (px[m - 1] - px[0]).hypot(py[m - 1] - py[0]) < 100.0;
    let margin = if closed { 3 } else { 1 };

    // Phase 1 — caps des segments (trace immobile → []).
    let headings = compute_headings(px, py);
    if headings.is_empty() {
        return Vec::new();
    }

    // Phase 2 — candidats sommets.
    let candidates = find_candidates(px, py, &headings, margin, params);

    // Phase 3 — groupage + représentant médian.
    let peaks = group_candidates(&candidates, &headings, px, py, params);

    // Phase 4/5 — paires miroirs + publication.
    let mut findings: Vec<Finding> = Vec::new();
    let mut prev_end: Option<usize> = None;

    for c in peaks {
        let i = c.i;
        let pairs = find_mirror_pairs(i, px, py, params, prev_end);
        let k = pairs.len();
        let s = i - k - 1;
        let e = i + k + 1;

        // Filtre prevEnd : emprise incluse dans la précédente → ignorée.
        if prev_end.map_or(false, |pe| s <= pe) {
            continue;
        }

        let ctx = FindingContext {
            up: if s >= 1 { Some(s - 1) } else { None },
            dn: if e + 1 <= m - 1 { Some(e + 1) } else { None },
        };

        let pair_txt = pairs
            .iter()
            .map(|p| {
                let dist = if p.d < 10.0 {
                    format!("{:.1}", p.d)
                } else {
                    format!("{}", p.d.round())
                };
                format!("pts {}↔{} ({} m)", p.a + 1, p.b + 1, dist)
            })
            .collect::<Vec<_>>()
            .join(" · ");

        // Numérotation par famille : le JS la fait à la fusion (ar + rp) ; en
        // Phase 1 (AR seul), elle est appliquée ici pour respecter le contrat.
        let n = findings.len() + 1;

        findings.push(Finding {
            id: format!("ar-{}", n),
            kind: FindingKind::Ar,
            label: format!("Aller-retour : {}", n),
            summary: format!(
                "sommet pt {} · {} paire{} · écart {}°",
                i + 1,
                k,
                if k > 1 { "s" } else { "" },
                format!("{:.0}", 180.0 - c.diff)
            ),
            peak: i,
            peak_id: ids[i],
            pairs: pairs
                .iter()
                .map(|p| FindingPair {
                    aid: ids[p.a],
                    bid: ids[p.b],
                    a: p.a,
                    b: p.b,
                    d: p.d,
                })
                .collect(),
            pair_idx: pairs.iter().flat_map(|p| [p.a, p.b]).collect(),
            ecart: Some(180.0 - c.diff),
            d1: Some(c.d1),
            d2: Some(c.d2),
            total_angle: None,
            turn_text: None,
            core_ids: Vec::new(),
            zone_ids: ids[s..=e].to_vec(),
            ctx_ids: FindingContextIds {
                up: ctx.up.map(|u| ids[u]),
                dn: ctx.dn.map(|d| ids[d]),
            },
            ctx,
            parts: vec![
                FindingPart {
                    s,
                    e,
                    role: PartRole::Warn,
                    text: if k > 0 {
                        format!("paires miroirs : {}", pair_txt)
                    } else {
                        "retournement isolé, aucune paire miroir".to_string()
                    },
                },
                FindingPart {
                    s: i - 1,
                    e: i + 1,
                    role: PartRole::Info,
                    text: format!(
                        "cœur : demi-tour · branches {}/{} m",
                        format!("{:.0}", c.d1),
                        format!("{:.0}", c.d2)
                    ),
                },
            ],
            status: FindingStatus::Pending,
            correction: None,
            undo: None,
        });

        prev_end = Some(e);
    }

    findings
}
