//! Pipeline de détection des tunnels.
//!
//! Portage de l'algorithme approuvé (`features/altimetrie/tunnels/Algorithmes.md`
//! §8) sans dépendance UI :
//! 1. lecture des points GPX et distance cumulée 2D ;
//! 2. cap GPX sur une fenêtre glissante ;
//! 3. projection locale, segments OSM et R-tree ;
//! 4. appariement point → segment (distance + cap, score `2·distance + angle`) ;
//! 5. runs contigus par `way_id`, filtrés par longueur minimale ;
//! 6. conversion en `Tunnel` et tri par kilomètre d'entrée.
//!
//! Le résultat est un `TunnelFile` avec `detectedAt` renseigné ; les statuts de
//! validation sont posés par `commands` dans le registre des traces.

use std::path::Path;

use rstar::{RTree, RTreeObject, AABB};

use super::geometry::{
    angle_diff_deg, barycenter, cap_at, cumulative_distances, point_to_segment, project_local,
    segment_cap_deg,
};
use super::pbf::OsmWay;
use super::types::{PbfFileRef, Tunnel, TunnelFile, TunnelParams, FILE_VERSION};

/// Lit les points d'un GPX dans l'ordre du fichier, en `[lon, lat]`.
///
/// L'altitude n'est pas requise : la détection des tunnels ne s'appuie que sur
/// la géométrie horizontale.
pub fn load_points(gpx_path: &Path) -> Result<Vec<[f64; 2]>, String> {
    if !gpx_path.exists() {
        return Err(format!(
            "Fichier GPX introuvable : {}.",
            gpx_path.display()
        ));
    }
    let file = std::fs::File::open(gpx_path)
        .map_err(|e| format!("Ouverture du GPX ({}): {}", gpx_path.display(), e))?;
    let gpx = gpx::read(std::io::BufReader::new(file))
        .map_err(|e| format!("GPX illisible ({}): {}", gpx_path.display(), e))?;

    let mut points = Vec::new();
    for track in &gpx.tracks {
        for segment in &track.segments {
            for waypoint in &segment.points {
                let point = waypoint.point();
                if !point.x().is_finite() || !point.y().is_finite() {
                    continue;
                }
                points.push([point.x(), point.y()]);
            }
        }
    }

    if points.len() < 2 {
        return Err("Trace trop courte : moins de 2 points exploitables.".to_string());
    }
    Ok(points)
}

/// Segment OSM projeté, indexé dans le R-tree.
#[derive(Debug, Clone, Copy)]
struct MatchSegment {
    way_index: usize,
    ax: f64,
    ay: f64,
    bx: f64,
    by: f64,
    cap: f64,
    envelope: AABB<[f64; 2]>,
}

impl RTreeObject for MatchSegment {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

/// Meilleur appariement d'un point GPX.
#[derive(Debug, Clone, Copy)]
struct BestMatch {
    way_index: usize,
    distance: f64,
    angle: f64,
    score: f64,
}

/// Run contigu de points appariés à la même way.
#[derive(Debug, Clone, Copy)]
struct Run {
    way_index: usize,
    start_idx: usize,
    end_idx: usize,
    max_distance: f64,
    max_angle: f64,
}

/// Détecte les tunnels d'une trace face aux ways OSM retenues.
pub fn match_tunnels(
    points: &[[f64; 2]],
    ways: &[OsmWay],
    params: &TunnelParams,
) -> Vec<Tunnel> {
    if points.len() < 2 || ways.is_empty() {
        return Vec::new();
    }

    let reference = barycenter(points);
    let cumul = cumulative_distances(points);
    let projected: Vec<(f64, f64)> = points
        .iter()
        .map(|point| project_local(*point, reference))
        .collect();
    let caps: Vec<f64> = (0..points.len())
        .map(|index| cap_at(points, &cumul, index, params.bearing_window_m))
        .collect();

    let mut segments = Vec::new();
    for (way_index, way) in ways.iter().enumerate() {
        if !way.tags.contains_key("highway") {
            continue;
        }
        for pair in way.coords.windows(2) {
            let (ax, ay) = project_local(pair[0], reference);
            let (bx, by) = project_local(pair[1], reference);
            if (bx - ax).hypot(by - ay) < 0.5 {
                continue;
            }
            segments.push(MatchSegment {
                way_index,
                ax,
                ay,
                bx,
                by,
                cap: segment_cap_deg(ax, ay, bx, by),
                envelope: AABB::from_corners(
                    [ax.min(bx), ay.min(by)],
                    [ax.max(bx), ay.max(by)],
                ),
            });
        }
    }
    if segments.is_empty() {
        return Vec::new();
    }

    let tree = RTree::bulk_load(segments);
    let distance_tolerance = params.distance_tolerance_m.max(0.0);
    let angle_tolerance = params.angle_tolerance_deg.max(0.0);
    let mut matches: Vec<Option<BestMatch>> = Vec::with_capacity(points.len());

    for (index, (px, py)) in projected.iter().enumerate() {
        let query = AABB::from_corners(
            [px - distance_tolerance, py - distance_tolerance],
            [px + distance_tolerance, py + distance_tolerance],
        );
        let mut best: Option<BestMatch> = None;

        for segment in tree.locate_in_envelope_intersecting(&query) {
            let distance = point_to_segment(
                *px,
                *py,
                segment.ax,
                segment.ay,
                segment.bx,
                segment.by,
            );
            if distance > distance_tolerance {
                continue;
            }
            let angle = angle_diff_deg(caps[index], segment.cap)
                .min(angle_diff_deg(caps[index], (segment.cap + 180.0) % 360.0));
            if angle > angle_tolerance {
                continue;
            }
            let score = 2.0 * distance + angle;
            if best.is_none_or(|candidate| score < candidate.score) {
                best = Some(BestMatch {
                    way_index: segment.way_index,
                    distance,
                    angle,
                    score,
                });
            }
        }

        matches.push(best);
    }

    let runs = collect_runs(&matches);
    let minimum_points = params.min_run_length.max(1);
    let mut tunnels = Vec::new();

    for (index, run) in runs
        .into_iter()
        .filter(|run| run.end_idx.saturating_sub(run.start_idx) + 1 >= minimum_points)
        .enumerate()
    {
        let way = &ways[run.way_index];
        let start_km = round_to(cumul[run.start_idx] / 1000.0, 6);
        let end_km = round_to(cumul[run.end_idx] / 1000.0, 6);
        if end_km <= start_km {
            continue;
        }
        let length_m = round_to((cumul[run.end_idx] - cumul[run.start_idx]).max(0.0), 1);
        if length_m < params.min_length_m {
            continue;
        }

        tunnels.push(Tunnel {
            id: format!("t-{:03}", index + 1),
            way_id: Some(way.id.clone()),
            name: way
                .tags
                .get("tunnel:name")
                .or_else(|| way.tags.get("name"))
                .cloned(),
            highway: way.tags.get("highway").cloned(),
            tunnel_type: way.tags.get("tunnel").cloned(),
            start_km,
            end_km,
            length_m,
            detection_method: "osm".to_string(),
            max_distance_m: Some(round_to(run.max_distance, 1)),
            max_angle_diff_deg: Some(round_to(run.max_angle, 1)),
            group_id: None,
        });
    }

    tunnels.sort_by(|a, b| a.start_km.total_cmp(&b.start_km));
    tunnels
}

/// Construit le fichier `tunnels.json` à partir des tunnels retenus.
pub fn build_file(
    trace_id: &str,
    params: &TunnelParams,
    pbf_files: Vec<PbfFileRef>,
    tunnels: Vec<Tunnel>,
    detected_at: Option<String>,
    updated_at: &str,
) -> TunnelFile {
    TunnelFile {
        version: FILE_VERSION,
        trace_id: trace_id.to_string(),
        detected_at,
        updated_at: updated_at.to_string(),
        pbf_files,
        params: params.clone(),
        tunnels,
    }
}

fn collect_runs(matches: &[Option<BestMatch>]) -> Vec<Run> {
    let mut runs = Vec::new();
    let mut current: Option<Run> = None;

    for (index, matched) in matches.iter().enumerate() {
        match matched {
            Some(candidate) => match &mut current {
                Some(run) if run.way_index == candidate.way_index => {
                    run.end_idx = index;
                    run.max_distance = run.max_distance.max(candidate.distance);
                    run.max_angle = run.max_angle.max(candidate.angle);
                }
                _ => {
                    if let Some(run) = current.take() {
                        runs.push(run);
                    }
                    current = Some(Run {
                        way_index: candidate.way_index,
                        start_idx: index,
                        end_idx: index,
                        max_distance: candidate.distance,
                        max_angle: candidate.angle,
                    });
                }
            },
            None => {
                if let Some(run) = current.take() {
                    runs.push(run);
                }
            }
        }
    }

    if let Some(run) = current {
        runs.push(run);
    }
    runs
}

fn round_to(value: f64, decimals: i32) -> f64 {
    let factor = 10_f64.powi(decimals);
    (value * factor).round() / factor
}
