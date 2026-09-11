//! Aperçu d'une correction par suppression (IHM §8.2 pour l'AR, §9.2 pour le RP).
//!
//! L'aperçu est **prospectif** : il montre ce que la trace deviendra si la plage
//! courante est validée, sans jamais modifier la trace de travail. Il est
//! recalculé à chaque mouvement de curseur, comme `delUpdate` / `rpDelUpdate`
//! dans le HTML de référence.
//!
//! Deux modèles cohabitent :
//! - **AR** (emprise courte) : la plage `[start..=end]` est supprimée, le reste
//!   est conservé ;
//! - **RP** (emprise longue) : tout l'intérieur est supprimé, les curseurs
//!   **épargnent** — la plage réellement supprimée est `[début+1 .. fin−1]`.
//!
//! Restent ici les seules parties qui dépendent de la géométrie métrique :
//! l'identification des ancres de routage d'une boucle et le placement radial
//! des étiquettes des points conservés. Le reste (couleurs, popups) est dérivé
//! côté front à partir des indicateurs sémantiques retournés.

use serde::{Deserialize, Serialize};

use super::geometry::{build_geometry, Projector};
use super::overlay::{radial_offsets, rp_anchors, LabelItem, CORE_SAMPLE_MAX};
use super::types::{AuditPoint, Finding, FindingKind, LatLon};

/// État de prévisualisation d'un point de l'emprise.
///
/// `kept` commande le bleu (« sera conservé ») ou le gris clair (« sera
/// supprimé ») ; `is_anchor` signale une ancre de routage RP, **destylisée**
/// pendant la vue (rayon et popup neutres).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewPointStyle {
    pub index: usize,
    pub kept: bool,
    pub is_anchor: bool,
}

/// Curseur de slider posé sur la carte (marqueur HTML).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewCursor {
    pub index: usize,
    pub lat: f64,
    pub lon: f64,
    /// Infobulle native : « conservé » ou « bord de trace » (AR aux extrémités).
    pub title: String,
}

/// Aperçu complet d'une suppression.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletePreview {
    /// Bornes effectives après clampage (le front les réécrit dans les curseurs).
    pub start: usize,
    pub end: usize,
    /// État de chaque point de l'emprise.
    pub points: Vec<PreviewPointStyle>,
    /// Chemin bleu des points conservés (`lyr-del-join`).
    pub join: Vec<LatLon>,
    /// Trait rouge dynamique de la plage supprimée (`lyr-del-red`) — RP seul.
    pub red: Vec<LatLon>,
    /// Curseurs de sliders à poser sur la carte.
    pub cursors: Vec<PreviewCursor>,
    /// Étiquettes des points conservés — RP seul (classe `keep`).
    pub labels: Vec<LabelItem>,
    /// Libellé du curseur Début (compteur AR ou intervalle conservé RP).
    pub count_start: String,
    /// Libellé du curseur Fin.
    pub count_end: String,
}

/// Calcule l'aperçu d'une suppression sur `[start..=end]`.
///
/// Les bornes sont **clampées** dans l'emprise de l'anomalie selon les règles de
/// `updateDeleteSelUI` (AR : `start ≤ end` ; RP : `fin ≥ début + 1`), puis
/// retournées dans `start`/`end` — le front les réécrit dans les curseurs, comme
/// le fait la référence.
pub fn delete_preview(
    points: &[AuditPoint],
    finding: &Finding,
    start: usize,
    end: usize,
    close_m: f64,
) -> Result<DeletePreview, String> {
    let part0 = finding
        .parts
        .first()
        .ok_or_else(|| "Anomalie sans emprise : aperçu impossible.".to_string())?;
    let (s0, e0) = (part0.s, part0.e);
    if points.is_empty() {
        return Err("Trace vide : aucun aperçu possible.".to_string());
    }
    if e0 >= points.len() {
        return Err("Emprise hors de la trace : aperçu impossible.".to_string());
    }

    if finding.kind == FindingKind::Rp {
        let part1 = finding
            .parts
            .get(1)
            .ok_or_else(|| "Anomalie RP sans cœur : aperçu impossible.".to_string())?;
        preview_rp(points, s0, e0, part1.s, part1.e, start, end, close_m)
    } else {
        preview_ar(points, s0, e0, start, end)
    }
}

/// Aperçu d'une suppression AR (portage de `delUpdate`).
fn preview_ar(
    points: &[AuditPoint],
    s0: usize,
    e0: usize,
    start: usize,
    end: usize,
) -> Result<DeletePreview, String> {
    let m = points.len();

    // Clamp : la plage reste dans l'emprise, et une plage inversée est ramenée
    // à un point isolé (`if (start > end) start = end` de `updateDeleteSelUI`).
    let mut start = start.clamp(s0, e0);
    let end = end.clamp(s0, e0);
    if start > end {
        start = end;
    }

    let mut styles = Vec::with_capacity(e0 - s0 + 1);
    for i in s0..=e0 {
        styles.push(PreviewPointStyle {
            index: i,
            kept: i < start || i > end,
            is_anchor: false,
        });
    }

    // Chemin bleu : du contexte amont jusqu'au dernier conservé amont, puis du
    // premier conservé aval jusqu'au contexte aval. Une seule polyligne — le
    // raccord direct à travers la plage supprimée est voulu.
    let a_idx = start as isize - 1;
    let b_idx = end + 1;
    let mut join: Vec<LatLon> = Vec::new();
    if a_idx >= 0 {
        for i in s0.saturating_sub(1)..=a_idx as usize {
            push_point(&mut join, points, i);
        }
    }
    if b_idx < m {
        for i in b_idx..=(e0 + 1).min(m - 1) {
            push_point(&mut join, points, i);
        }
    }
    if join.len() < 2 {
        join.clear();
    }

    // Curseurs : posés sur les points **conservés** adjacents, avec repli sur la
    // borne supprimée quand la plage touche un bout de trace.
    let c_a = if a_idx >= 0 { a_idx as usize } else { start };
    let c_b = if b_idx < m { b_idx } else { end };
    let cursors = vec![
        cursor_at(points, c_a, if a_idx >= 0 { "conservé" } else { "bord de trace" }),
        cursor_at(points, c_b, if b_idx < m { "conservé" } else { "bord de trace" }),
    ];

    Ok(DeletePreview {
        start,
        end,
        points: styles,
        join,
        red: Vec::new(),
        cursors,
        labels: Vec::new(),
        count_start: format!("pt {}", start + 1),
        count_end: format!("pt {}", end + 1),
    })
}

/// Aperçu d'une suppression RP (portage de `rpDelUpdate` + `rpDelLabels`).
///
/// Les curseurs **épargnent** : tout l'intérieur de l'emprise est supprimé par
/// défaut, seul ce qui précède le curseur Début et ce qui suit le curseur Fin
/// est conservé.
#[allow(clippy::too_many_arguments)]
fn preview_rp(
    points: &[AuditPoint],
    s0: usize,
    e0: usize,
    cs: usize,
    ce: usize,
    start: usize,
    end: usize,
    close_m: f64,
) -> Result<DeletePreview, String> {
    // Clamp de `updateDeleteSelUI` : les curseurs ne se croisent jamais et
    // laissent au moins un point supprimable entre eux.
    let debut = start.clamp(s0, e0);
    let mut fin = end.clamp(s0, e0);
    let debut = if (debut as i64) > fin as i64 - 1 {
        (fin as i64 - 1).max(s0 as i64) as usize
    } else {
        debut
    };
    if (fin as i64) < debut as i64 + 1 {
        fin = (debut + 1).min(e0);
    }

    // Ancres de routage de la boucle (destylisées pendant l'aperçu).
    let geo = build_geometry(points);
    let proj = Projector::new(&points[0]);
    let anchors = rp_anchors(&geo.px, &geo.py, &proj, cs, ce, s0, e0, close_m);

    let mut styles = Vec::with_capacity(e0 - s0 + 1);
    for i in s0..=e0 {
        styles.push(PreviewPointStyle {
            index: i,
            kept: i <= debut || i >= fin,
            is_anchor: i == anchors.up || i == anchors.dn,
        });
    }

    // Chemin bleu : segments réels conservés, avec la corde directe entre les
    // deux curseurs (le raccord qui « parle » à la validation).
    let mut join: Vec<LatLon> = Vec::new();
    for i in s0..=debut {
        push_point(&mut join, points, i);
    }
    for i in fin..=e0 {
        push_point(&mut join, points, i);
    }

    // Trait rouge : la plage réellement supprimée, le long de la trace.
    let mut red: Vec<LatLon> = Vec::new();
    if debut + 1 <= fin.saturating_sub(1) {
        for i in debut + 1..=fin - 1 {
            push_point(&mut red, points, i);
        }
    }
    if red.len() < 2 {
        red.clear();
    }

    let cursors = vec![
        cursor_at(points, debut, "conservé"),
        cursor_at(points, fin, "conservé"),
    ];

    // Étiquettes des points conservés, placement radial autour du centre du cœur.
    let mut kept: Vec<usize> = Vec::new();
    for i in s0..=debut {
        kept.push(i);
    }
    for i in fin..=e0 {
        kept.push(i);
    }
    let labels = keep_labels(points, &geo.px, &geo.py, cs, ce, &kept);

    Ok(DeletePreview {
        start: debut,
        end: fin,
        points: styles,
        join,
        red,
        cursors,
        labels,
        count_start: format!("Points conservés : [{} – {}]", s0 + 1, debut + 1),
        count_end: format!("Points conservés : [{} – {}]", fin + 1, e0 + 1),
    })
}

/// Étiquettes des points conservés (portage de `rpDelLabels`).
///
/// Un label par point conservé, classe `keep`, placement radial autour du
/// centroïde du cœur — même règle que les callouts RP.
fn keep_labels(
    points: &[AuditPoint],
    px: &[f64],
    py: &[f64],
    cs: usize,
    ce: usize,
    kept: &[usize],
) -> Vec<LabelItem> {
    if kept.is_empty() {
        return Vec::new();
    }

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

    let pts: Vec<(f64, f64)> = kept
        .iter()
        .map(|&k| {
            (
                px.get(k).copied().unwrap_or(f64::NAN),
                py.get(k).copied().unwrap_or(f64::NAN),
            )
        })
        .collect();
    let offs = radial_offsets(&pts, cxm, cym);

    kept.iter()
        .enumerate()
        .filter_map(|(r, &k)| {
            let p = points.get(k)?;
            Some(LabelItem {
                index: k,
                lat: p.lat,
                lon: p.lon,
                no: k + 1,
                cls: "keep".to_string(),
                dx: offs[r].0,
                dy: offs[r].1,
            })
        })
        .collect()
}

/// Ajoute le point `i` au chemin s'il existe (jamais de panic hors bornes).
fn push_point(path: &mut Vec<LatLon>, points: &[AuditPoint], i: usize) {
    if let Some(p) = points.get(i) {
        path.push(LatLon {
            lat: p.lat,
            lon: p.lon,
        });
    }
}

/// Curseur de slider posé sur le point `i` (jamais de panic hors bornes).
fn cursor_at(points: &[AuditPoint], i: usize, title: &str) -> PreviewCursor {
    let p = points.get(i);
    PreviewCursor {
        index: i,
        lat: p.map(|p| p.lat).unwrap_or(f64::NAN),
        lon: p.map(|p| p.lon).unwrap_or(f64::NAN),
        title: title.to_string(),
    }
}
