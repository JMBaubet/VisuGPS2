//! Éléments de rendu des étiquettes et des ancres de routage (IHM §5.1, §5.4).
//!
//! Ces calculs sont **métriques** (tangente locale, normale extérieure, centre
//! du cercle ajusté) : ils restent donc en Rust, au même titre que les
//! détecteurs, et le front se contente de créer les marqueurs DOM aux positions
//! reçues. C'est le pendant de `showStandardLabels` / `rpAnchors` du HTML de
//! référence, qui recalcule ces éléments **à chaque rendu** (`lazy`).

use serde::{Deserialize, Serialize};

use super::anchor::rp_anchor_indices;
use super::geometry::{build_geometry, Projector};
use super::types::{AuditPoint, Finding, FindingKind, FindingStatus, LatLon, UndoRecord};

/// Plafond du nombre d'étiquettes par anomalie (`LABEL_MAX` du JS).
pub const LABEL_MAX: usize = 120;

/// Distance sous laquelle deux points sont « jumeaux métriques » et décalés
/// d'une couronne supplémentaire (`LABEL_DUP_M` du JS).
pub const LABEL_DUP_M: f64 = 8.0;

/// Nombre maximal de points échantillonnés pour le centre du cœur RP
/// (`RP_ANCHOR_MAX_SAMPLES` du JS, ici appliqué à la moyenne du cœur).
/// Partagé avec l'aperçu de suppression, qui reprend le même centroïde.
pub(super) const CORE_SAMPLE_MAX: usize = 200;

/// Ancres de routage d'une boucle RP (IHM §5.1, « Calcul lazy »).
///
/// `up`/`dn` sont les indices **clampés à l'emprise** du finding, comme le fait
/// `rpAnchors` côté JS ; `center`/`radius_m` décrivent le cercle ajusté (croix
/// Kåsa).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpAnchors {
    pub up: usize,
    pub dn: usize,
    /// `true` si au moins une ancre provient de l'extension radiale (et non du
    /// repli `cs − 1` / `ce + 1`).
    pub natural: bool,
    pub center: Option<LatLon>,
    pub radius_m: Option<f64>,
}

/// Étiquette d'un point : position géographique, numéro affiché, classe de
/// style et décalage initial (en pixels écran) par rapport au point.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelItem {
    pub index: usize,
    pub lat: f64,
    pub lon: f64,
    pub no: usize,
    pub cls: String,
    pub dx: f64,
    pub dy: f64,
}

/// Éléments de rendu d'une anomalie sélectionnée.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapOverlay {
    /// Ancres de routage — renseignées pour une boucle RP non corrigée.
    pub anchors: Option<RpAnchors>,
    /// Étiquettes, dans l'ordre d'affichage.
    pub labels: Vec<LabelItem>,
}

/// Éléments de rendu d'une anomalie, identifiés par son identifiant stable.
///
/// Le rendu carte a besoin des **ancres de toutes** les boucles (elles colorent
/// les points d'ancre de chaque emprise) et des étiquettes de celle qui est
/// sélectionnée : tout est donc produit en un appel, à l'image du HTML de
/// référence qui recalcule `rpAnchors` pour chaque finding à chaque rendu.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindingOverlay {
    pub id: String,
    pub anchors: Option<RpAnchors>,
    pub labels: Vec<LabelItem>,
}

// ─── Ancres ───────────────────────────────────────────────────────────

/// Ancres de routage clampées (portage de `rpAnchors`, IHM §5.1).
///
/// `junc` est la jonction de la boucle (= `parts[1].s`, égale à `peak`) ; `s`/`e`
/// bornent l'emprise du finding.
pub fn rp_anchors(
    px: &[f64],
    py: &[f64],
    proj: &Projector,
    junc: usize,
    ce: usize,
    s: usize,
    e: usize,
    close_m: f64,
) -> RpAnchors {
    let an = rp_anchor_indices(px, py, junc, ce, close_m);

    // Repli `cs − 1` / `ce + 1` quand l'extension radiale n'a rien trouvé, puis
    // clampage dans l'emprise du finding (`Math.max(s, Math.min(up, cs-1))`).
    let fallback_up = junc.saturating_sub(1);
    let up = s.max(an.up.unwrap_or(fallback_up).min(fallback_up));
    let dn = e.min(an.dn.unwrap_or(ce + 1).max(ce + 1));

    let center = match (an.cx, an.cy) {
        (Some(cx), Some(cy)) => {
            let (lat, lon) = proj.inv(cx, cy);
            Some(LatLon { lat, lon })
        }
        _ => None,
    };

    RpAnchors {
        up,
        dn,
        natural: an.up.is_some() || an.dn.is_some(),
        center,
        radius_m: an.r,
    }
}

// ─── Placement ────────────────────────────────────────────────────────

/// Décalages radiaux des étiquettes d'une boucle (portage de `radialOffsets`).
///
/// Normale extérieure à la tangente locale, en quinconce 60/84 px, avec une
/// couronne de +24 px par jumeau métrique à moins de `LABEL_DUP_M`. Le repli
/// radial sert quand la tangente est indéterminée (points confondus).
///
/// `dup` compte les points **déjà parcourus** à moins de `LABEL_DUP_M` — l'ordre
/// de parcours fait donc partie du résultat (portage littéral).
pub(super) fn radial_offsets(pts: &[(f64, f64)], cxm: f64, cym: f64) -> Vec<(f64, f64)> {
    let n = pts.len();
    if n == 0 {
        return Vec::new();
    }

    let mut placed: Vec<(f64, f64)> = Vec::with_capacity(n);
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(n);

    for (r, &(x, y)) in pts.iter().enumerate() {
        let (pax, pay) = pts[if r > 0 { r - 1 } else { 0 }];
        let (pbx, pby) = pts[if r + 1 < n { r + 1 } else { n - 1 }];

        let mut tx = pbx - pax;
        let mut ty = pby - pay;
        let tl = tx.hypot(ty);

        let (nx, ny) = if tl < 1e-6 {
            // Tangente indéterminée : repli radial depuis le centre.
            let mut nx = x - cxm;
            let mut ny = y - cym;
            let nl = nx.hypot(ny);
            let nl = if nl == 0.0 { 1.0 } else { nl };
            nx /= nl;
            ny /= nl;
            (nx, ny)
        } else {
            tx /= tl;
            ty /= tl;
            let mut nx = ty;
            let mut ny = -tx;
            // Orientée vers l'extérieur du cercle.
            if nx * (x - cxm) + ny * (y - cym) < 0.0 {
                nx = -nx;
                ny = -ny;
            }
            (nx, ny)
        };

        let mut dup = 0usize;
        for &(qx, qy) in &placed {
            if (qx - x).hypot(qy - y) < LABEL_DUP_M {
                dup += 1;
            }
        }
        placed.push((x, y));

        let base = if r % 2 == 1 { 84.0 } else { 60.0 };
        let dist = base + 24.0 * dup as f64;
        out.push((nx * dist, -ny * dist));
    }

    out
}

/// Construit une étiquette pour l'indice `i` de la trace de travail.
///
/// Retourne `None` si l'indice sort de la trace (jamais de panic, jamais de
/// coordonnée non finie dans le GeoJSON rendu).
fn label_at(points: &[AuditPoint], i: usize, cls: &str, dx: f64, dy: f64) -> Option<LabelItem> {
    let p = points.get(i)?;
    Some(LabelItem {
        index: i,
        lat: p.lat,
        lon: p.lon,
        no: i + 1,
        cls: cls.to_string(),
        dx,
        dy,
    })
}

/// Éventail des étiquettes d'un aller-retour (portage de `fanItems`).
///
/// Offsets croissants par rang de part et d'autre du sommet, sommet à `(0, −84)`
/// et points de contexte en prolongement.
fn fan_items(
    points: &[AuditPoint],
    s: usize,
    e: usize,
    peak: usize,
    cls: &str,
    with_ctx: bool,
    ctx_up: Option<usize>,
    ctx_dn: Option<usize>,
) -> Vec<LabelItem> {
    let mut items: Vec<LabelItem> = Vec::new();

    for j in s..=e {
        if j == peak {
            continue;
        }
        let r = j.abs_diff(peak);
        let sign = if j < peak { -1.0 } else { 1.0 };
        let dy = if r % 2 == 1 { 34.0 } else { 58.0 };
        if let Some(it) = label_at(points, j, cls, sign * (20.0 * r as f64 + 8.0), sign * dy) {
            items.push(it);
        }
    }

    // Le sommet porte la classe `pk` (ou `fpl` pour un faux positif), au-dessus
    // de l'éventail.
    if let Some(it) = label_at(
        points,
        peak,
        if cls == "fpl" { "fpl" } else { "pk" },
        0.0,
        -84.0,
    ) {
        items.push(it);
    }

    if with_ctx {
        if let Some(cu) = ctx_up {
            if let Some(it) = label_at(
                points,
                cu,
                "ctx",
                -(20.0 * (peak - s + 2) as f64 + 8.0),
                -34.0,
            ) {
                items.push(it);
            }
        }
        if let Some(cd) = ctx_dn {
            if let Some(it) = label_at(
                points,
                cd,
                "ctx",
                20.0 * (e - peak + 2) as f64 + 8.0,
                34.0,
            ) {
                items.push(it);
            }
        }
    }

    items
}

/// Étiquettes d'une boucle RP (portage de `rpLabelItems`).
///
/// Un point sur `k` au-delà de `LABEL_MAX` (stride), avec étiquetage
/// systématique des jalons : bornes d'emprise, ancres et jonction.
#[allow(clippy::too_many_arguments)]
fn rp_label_items(
    points: &[AuditPoint],
    px: &[f64],
    py: &[f64],
    s: usize,
    e: usize,
    cs: usize,
    ce: usize,
    peak: usize,
    is_fp: bool,
    anchors: &RpAnchors,
) -> Vec<LabelItem> {
    // Centre du cœur — moyenne du cœur échantillonné (pas plafonné).
    let cstep = (ce - cs + 1).div_ceil(CORE_SAMPLE_MAX).max(1);
    let mut cxm = 0.0;
    let mut cym = 0.0;
    let mut cnt = 0usize;
    let mut i = cs;
    while i <= ce {
        if let (Some(x), Some(y)) = (px.get(i), py.get(i)) {
            if x.is_finite() && y.is_finite() {
                cxm += x;
                cym += y;
                cnt += 1;
            }
        }
        i += cstep;
    }
    if cnt > 0 {
        cxm /= cnt as f64;
        cym /= cnt as f64;
    }

    // Sélection des indices étiquetés : stride global, puis jalons.
    let count = e - s + 1;
    let stride = if count > LABEL_MAX {
        count.div_ceil(LABEL_MAX)
    } else {
        1
    };
    let mut idxs: Vec<usize> = Vec::new();
    let mut j = s;
    while j <= e {
        idxs.push(j);
        j += stride;
    }
    for k in [s, anchors.up, peak, anchors.dn, e] {
        if k >= s && k <= e && !idxs.contains(&k) {
            idxs.push(k);
        }
    }
    idxs.sort_unstable();

    let pts: Vec<(f64, f64)> = idxs
        .iter()
        .map(|&k| {
            (
                px.get(k).copied().unwrap_or(f64::NAN),
                py.get(k).copied().unwrap_or(f64::NAN),
            )
        })
        .collect();
    let offs = radial_offsets(&pts, cxm, cym);
    let cls = if is_fp { "fpl" } else { "" };

    idxs.iter()
        .enumerate()
        .filter_map(|(r, &k)| label_at(points, k, cls, offs[r].0, offs[r].1))
        .collect()
}

/// Étiquettes des **points d'origine** d'une anomalie corrigée (§5.4).
///
/// AR : offsets alternés ; RP : règle radiale sur les points d'origine, classe
/// `del` uniforme, ex-jonction toujours étiquetée, stride si > `LABEL_MAX`.
fn corrected_labels(orig: &[AuditPoint], finding: &Finding, first_no: usize) -> Vec<LabelItem> {
    if orig.is_empty() {
        return Vec::new();
    }

    if finding.kind == FindingKind::Rp {
        // Les points d'origine sont reprojetés depuis leurs propres coordonnées
        // (ils ne sont plus dans la trace de travail).
        let proj = Projector::new(&orig[0]);
        let pts: Vec<(f64, f64)> = orig.iter().map(|p| proj.fwd(p)).collect();
        let mut cxm = 0.0;
        let mut cym = 0.0;
        for &(x, y) in &pts {
            cxm += x;
            cym += y;
        }
        cxm /= pts.len() as f64;
        cym /= pts.len() as f64;

        // Rang de l'ex-jonction, retrouvé par son identifiant stable.
        let junc_rank = orig
            .iter()
            .position(|p| p.id == finding.peak_id)
            .map(|r| r as isize)
            .unwrap_or(-1);

        let stride = if pts.len() > LABEL_MAX {
            pts.len().div_ceil(LABEL_MAX)
        } else {
            1
        };
        let mut ranks: Vec<usize> = (0..pts.len()).step_by(stride).collect();
        for r in [0isize, junc_rank, pts.len() as isize - 1] {
            if r >= 0 && !ranks.contains(&(r as usize)) {
                ranks.push(r as usize);
            }
        }
        ranks.sort_unstable();

        let sel: Vec<(f64, f64)> = ranks.iter().map(|&r| pts[r]).collect();
        let offs = radial_offsets(&sel, cxm, cym);

        return ranks
            .iter()
            .enumerate()
            .map(|(k, &r)| LabelItem {
                index: r,
                lat: orig[r].lat,
                lon: orig[r].lon,
                no: first_no + r,
                cls: "del".to_string(),
                dx: offs[k].0,
                dy: offs[k].1,
            })
            .collect();
    }

    orig.iter()
        .enumerate()
        .map(|(rank, p)| LabelItem {
            index: rank,
            lat: p.lat,
            lon: p.lon,
            no: first_no + rank,
            cls: "del".to_string(),
            dx: if rank % 2 == 1 { -1.0 } else { 1.0 } * (24.0 * (rank as f64 + 1.0)),
            dy: -50.0 - 22.0 * (rank % 3) as f64,
        })
        .collect()
}

// ─── Point d'entrée ───────────────────────────────────────────────────

/// Éléments de rendu d'une anomalie (portage de `showStandardLabels` et de
/// `rpAnchors`).
///
/// `points` est la trace de travail courante : la géométrie métrique est
/// reconstruite à l'identique de la détection (même origine de projection), ce
/// qui rend les indices portés par le finding interprétables.
pub fn map_overlay(
    points: &[AuditPoint],
    finding: &Finding,
    close_m: f64,
) -> Result<MapOverlay, String> {
    let first = points
        .first()
        .ok_or_else(|| "Trace vide : aucun rendu possible.".to_string())?;
    let proj = Projector::new(first);

    // Une anomalie corrigée n'a ni ancres ni étiquettes de détection : ses
    // étiquettes portent sur les **points d'origine** (classe `del`).
    if finding.status == FindingStatus::Corrected {
        let (orig, first_no) = match finding.undo.as_ref() {
            Some(UndoRecord::Delete(u)) => (u.orig_pts.clone(), u.first_no),
            Some(UndoRecord::Route(u)) => (u.orig_pts.clone(), u.first_no),
            None => (Vec::new(), 1),
        };
        return Ok(MapOverlay {
            anchors: None,
            labels: corrected_labels(&orig, finding, first_no),
        });
    }

    let part0 = finding
        .parts
        .first()
        .ok_or_else(|| "Anomalie sans emprise : rendu impossible.".to_string())?;
    let (s, e) = (part0.s, part0.e);

    if finding.kind == FindingKind::Ar {
        // Éventail AR : tous les points de l'emprise, plus le contexte sain.
        let labels = fan_items(
            points,
            s,
            e,
            finding.peak,
            if finding.status == FindingStatus::Fp {
                "fpl"
            } else {
                ""
            },
            finding.status == FindingStatus::Pending,
            finding.ctx.up,
            finding.ctx.dn,
        );
        return Ok(MapOverlay {
            anchors: None,
            labels,
        });
    }

    // Boucle RP : ancres de routage + étiquettes radiales.
    let part1 = finding
        .parts
        .get(1)
        .ok_or_else(|| "Anomalie RP sans cœur : rendu impossible.".to_string())?;
    let (cs, ce) = (part1.s, part1.e);
    let geo = build_geometry(points);

    let anchors = rp_anchors(&geo.px, &geo.py, &proj, cs, ce, s, e, close_m);
    let labels = rp_label_items(
        points,
        &geo.px,
        &geo.py,
        s,
        e,
        cs,
        ce,
        finding.peak,
        finding.status == FindingStatus::Fp,
        &anchors,
    );

    Ok(MapOverlay {
        anchors: Some(anchors),
        labels,
    })
}

/// Éléments de rendu de **toutes** les anomalies de l'audit (IHM §5.1).
///
/// Les anomalies dont le rendu échoue (emprise absente) sont simplement
/// ignorées : une anomalie mal formée ne doit pas priver la carte du rendu des
/// autres.
pub fn map_overlays(
    points: &[AuditPoint],
    findings: &[Finding],
    close_m: f64,
) -> Result<Vec<FindingOverlay>, String> {
    if points.is_empty() {
        return Err("Trace vide : aucun rendu possible.".to_string());
    }

    let mut out = Vec::with_capacity(findings.len());
    for f in findings {
        match map_overlay(points, f, close_m) {
            Ok(ov) => out.push(FindingOverlay {
                id: f.id.clone(),
                anchors: ov.anchors,
                labels: ov.labels,
            }),
            Err(e) => {
                eprintln!("[audit] rendu ignoré pour {} : {}", f.id, e);
            }
        }
    }
    Ok(out)
}
