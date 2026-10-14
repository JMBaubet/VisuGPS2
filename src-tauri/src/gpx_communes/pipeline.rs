//! Algorithme de détection des communes traversées.
//!
//! Portage du prototype validé (`features/communes/`) : simplification,
//! filtrage des candidates, échantillonnage, arbitrage par centroïde, comblement
//! des micro-trous, extraction et raffinement des passages, post-traitement,
//! numérotation.
//!
//! Un réglage du prototype n'est pas porté : `oscillationRatio`, désactivé par
//! défaut là-bas (`0`), donc sans effet — la passe de fusion des oscillations
//! n'est pas implémentée. `maxGapFillSamples` est une constante interne (voir
//! [`MAX_GAP_FILL_SAMPLES`]).

use std::collections::HashMap;

use super::geometry;
use super::osm::OsmCommune;
use super::types::{CommuneParams, CommunePassage, MAX_GAP_FILL_SAMPLES};

/// Commune candidate, enrichie pour l'analyse.
struct Candidate {
    name: String,
    polygons: Vec<PreparedPolygon>,
    /// Surface nette (m²) — filtre des polygones aberrants.
    area_m2: f64,
    /// Centroïde de la commune, pondéré par la surface de ses polygones ;
    /// critère d'arbitrage en cas de chevauchement.
    centroid: [f64; 2],
}

/// Polygone prêt pour l'analyse : bbox et centroïde précalculés.
struct PreparedPolygon {
    polygon: super::osm::OsmPolygon,
    bbox: [f64; 4],
    centroid: [f64; 2],
    area_m2: f64,
}

/// Passage en cours de construction, avant numérotation.
#[derive(Debug, Clone, PartialEq)]
struct DraftPassage {
    /// Index de la commune dans la liste des candidates.
    commune: usize,
    entry_km: f64,
    exit_km: f64,
}

impl DraftPassage {
    fn length_km(&self) -> f64 {
        self.exit_km - self.entry_km
    }
}

/// Détecte les passages dans les communes le long d'une trace.
///
/// `trace` : coordonnées `[lon, lat]` de la trace **nettoyée**, dans l'ordre du
/// parcours. `communes` : polygones issus d'Overpass. `params` : réglages
/// `Communes.Detection`.
///
/// Retourne les passages triés par kilomètre d'entrée, chacun portant son numéro
/// et le nombre total de passages de sa commune. Une liste vide est un
/// **résultat** — la trace ne traverse aucune commune connue — et non une erreur.
pub fn detect_passages(
    trace: &[[f64; 2]],
    communes: &[OsmCommune],
    params: &CommuneParams,
) -> Result<Vec<CommunePassage>, String> {
    if trace.len() < 2 {
        return Err("Trace dégénérée : moins de 2 points.".to_string());
    }

    // 1. Simplification : réduit le coût du filtrage et de la timeline sans
    //    déplacer la trace au-delà de la précision des polygones OSM.
    let line = geometry::simplify(trace, params.simplify_tolerance);
    let cumul = geometry::cumulative_distances(&line);
    let total_m = cumul.last().copied().unwrap_or(0.0);
    if total_m <= 0.0 {
        return Err("Trace dégénérée : longueur nulle.".to_string());
    }
    let length_km = total_m / 1000.0;

    // 2. Candidates : polygones effectivement croisés par la trace.
    let mut candidates: Vec<Candidate> = communes
        .iter()
        .filter(|commune| {
            commune
                .polygons
                .iter()
                .any(|poly| geometry::line_intersects_polygon(&line, poly))
        })
        .map(prepare)
        .collect();

    // 3. Filtre des polygones aberrants : un multipolygone OSM cassé a une
    //    surface démesurée qui englobe ses voisines.
    if !candidates.is_empty() {
        let mut areas: Vec<f64> = candidates.iter().map(|c| c.area_m2).collect();
        let threshold = median(&mut areas) * params.max_area_ratio;
        let all = std::mem::take(&mut candidates);
        let (kept, dropped): (Vec<Candidate>, Vec<Candidate>) =
            all.into_iter().partition(|c| c.area_m2 <= threshold);
        candidates = if kept.is_empty() {
            // Sécurité du prototype : mieux vaut une commune aberrante qu'une
            // trace sans aucune commune — le filtre ne peut pas tout emporter.
            dropped
        } else {
            kept
        };
    }
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    // 4. Échantillonnage à pas fixe, plafonné par le nombre cible d'échantillons.
    let step_km = (length_km / params.target_samples.max(1) as f64).max(params.min_sample_step_km);
    let step_count = (length_km / step_km).floor() as usize;
    let mut samples: Vec<(f64, [f64; 2])> = (0..=step_count)
        .map(|i| {
            let d_km = i as f64 * step_km;
            (d_km, geometry::point_at_distance(&line, &cumul, d_km * 1000.0))
        })
        .collect();
    // L'arrivée est toujours échantillonnée : une transition sur le dernier pas
    // de la trace serait sinon invisible.
    if samples.last().map(|(d, _)| *d).unwrap_or(0.0) < length_km {
        samples.push((length_km, line[line.len() - 1]));
    }

    // 5. Timeline : commune gagnante de chaque échantillon.
    let mut timeline: Vec<Option<usize>> = samples
        .iter()
        .map(|(_, point)| winner(&candidates, *point))
        .collect();

    // 6. Comblement des micro-trous.
    fill_gaps(&mut timeline, MAX_GAP_FILL_SAMPLES);

    // 7. Segments contigus, bornes raffinées par intersection de frontière.
    let mut drafts = extract_segments(&timeline, &samples, &candidates, length_km);

    // 8. Micro-passages : bruit de simplification ou de frontière OSM.
    drafts.retain(|draft| draft.length_km() >= params.min_segment_km);

    // 9. Post-traitement : passages courts absorbés, passages contigus recollés.
    merge_short_segments(&mut drafts, params.min_keep_km);
    merge_contiguous(&mut drafts);

    // 10. Tri chronologique (le parcours de la timeline suit déjà l'ordre, mais
    //     les fusions redistribuent les bornes).
    drafts.sort_by(|a, b| {
        a.entry_km
            .partial_cmp(&b.entry_km)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // 11. Numérotation des passages.
    Ok(number(drafts, &candidates))
}

/// Enrichit une commune reçue d'Overpass (bbox et centroïdes précalculés).
fn prepare(commune: &OsmCommune) -> Candidate {
    let polygons: Vec<PreparedPolygon> = commune
        .polygons
        .iter()
        .map(|polygon| PreparedPolygon {
            bbox: geometry::bbox(&polygon.outer),
            centroid: geometry::polygon_centroid(polygon),
            area_m2: geometry::polygon_area_m2(polygon),
            polygon: polygon.clone(),
        })
        .collect();

    let area_m2: f64 = polygons.iter().map(|p| p.area_m2).sum();
    // Centroïde d'un multipolygone : moyenne des centroïdes pondérée par les
    // surfaces (comportement de `turf.centroid`).
    let centroid = if area_m2 > 0.0 {
        let sx: f64 = polygons.iter().map(|p| p.centroid[0] * p.area_m2).sum();
        let sy: f64 = polygons.iter().map(|p| p.centroid[1] * p.area_m2).sum();
        [sx / area_m2, sy / area_m2]
    } else {
        polygons.first().map(|p| p.centroid).unwrap_or([0.0, 0.0])
    };

    Candidate {
        name: commune.name.clone(),
        polygons,
        area_m2,
        centroid,
    }
}

/// Commune gagnante d'un échantillon : celle dont un polygone contient le point.
///
/// En cas de chevauchement — polygone OSM défectueux débordant sur sa voisine —
/// c'est la commune dont le **centroïde** est le plus proche qui gagne : un
/// polygone fantôme a son centroïde au bon endroit, donc sur le territoire du
/// voisin c'est le voisin qui l'emporte. La règle ne dépend ni de la surface ni
/// de l'ordre d'énumération (résultat déterministe).
fn winner(candidates: &[Candidate], point: [f64; 2]) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (index, candidate) in candidates.iter().enumerate() {
        let inside = candidate.polygons.iter().any(|poly| {
            geometry::bbox_contains(poly.bbox, point)
                && geometry::point_in_polygon(&poly.polygon, point)
        });
        if !inside {
            continue;
        }
        let distance = geometry::haversine_m(point, candidate.centroid);
        if best.is_none_or(|(_, current)| distance < current) {
            best = Some((index, distance));
        }
    }
    best.map(|(index, _)| index)
}

/// Comble les trous de la timeline d'au plus `max_gap` échantillons, **flanqués
/// de la même commune** des deux côtés : un artefact de précision de frontière,
/// jamais une vraie transition.
pub(crate) fn fill_gaps(timeline: &mut [Option<usize>], max_gap: usize) {
    if max_gap == 0 {
        return;
    }
    let mut index = 0;
    while index < timeline.len() {
        if timeline[index].is_some() {
            index += 1;
            continue;
        }
        let gap_start = index;
        while index < timeline.len() && timeline[index].is_none() {
            index += 1;
        }
        if index - gap_start > max_gap {
            continue;
        }
        let before = if gap_start > 0 {
            timeline[gap_start - 1]
        } else {
            None
        };
        let after = if index < timeline.len() {
            timeline[index]
        } else {
            None
        };
        if let (Some(before), Some(after)) = (before, after) {
            if before == after {
                for slot in &mut timeline[gap_start..index] {
                    *slot = Some(before);
                }
            }
        }
    }
}

/// Extrait les segments contigus de la timeline, bornes raffinées.
fn extract_segments(
    timeline: &[Option<usize>],
    samples: &[(f64, [f64; 2])],
    candidates: &[Candidate],
    length_km: f64,
) -> Vec<DraftPassage> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut current: Option<usize> = None;

    // Le tour supplémentaire (`..=len`) referme le dernier segment.
    for index in 0..=timeline.len() {
        let winner = timeline.get(index).copied().flatten();
        if winner == current {
            continue;
        }

        if let (Some(commune), Some(first)) = (current, start) {
            let last = index - 1;
            let entry_km = if first == 0 {
                // Le segment commence au départ de la trace : aucun franchissement.
                0.0
            } else {
                refined_km(&candidates[commune], samples[first - 1], samples[first])
            };
            let exit_km = if last + 1 >= samples.len() {
                // Le segment se termine à l'arrivée : aucun franchissement.
                length_km
            } else {
                refined_km(&candidates[commune], samples[last], samples[last + 1])
            };
            out.push(DraftPassage {
                commune,
                entry_km,
                exit_km,
            });
        }

        start = winner.map(|_| index);
        current = winner;
    }

    out
}

/// Kilomètre raffiné d'un franchissement de frontière, entre deux échantillons
/// encadrants.
///
/// La position exacte du franchissement est l'intersection entre le petit
/// segment de trace (≤ un pas d'échantillonnage) et la frontière de la commune ;
/// elle est convertie en kilomètre par interpolation locale. C'est cette étape
/// qui donne une précision métrique là où le pas d'échantillonnage est de 100 m.
fn refined_km(candidate: &Candidate, before: (f64, [f64; 2]), after: (f64, [f64; 2])) -> f64 {
    let crossings: Vec<[f64; 2]> = candidate
        .polygons
        .iter()
        .flat_map(|poly| geometry::boundary_crossings(&poly.polygon, before.1, after.1))
        .collect();

    // Le franchissement retenu est le plus proche du point d'arrivée : dans un
    // recoin, une même frontière peut être franchie deux fois par un seul pas.
    let nearest = crossings.iter().min_by(|a, b| {
        geometry::haversine_m(**a, after.1)
            .partial_cmp(&geometry::haversine_m(**b, after.1))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    match nearest {
        Some(coord) => km_at(before, after, *coord),
        // Géométrie OSM inexploitable : le point d'arrivée fait approximation.
        None => after.0,
    }
}

/// Kilomètre d'un point situé entre deux échantillons encadrants, par
/// interpolation locale des distances.
fn km_at(before: (f64, [f64; 2]), after: (f64, [f64; 2]), coord: [f64; 2]) -> f64 {
    let to_before = geometry::haversine_m(before.1, coord);
    let to_after = geometry::haversine_m(coord, after.1);
    let total = to_before + to_after;
    if total < 1e-9 {
        return after.0;
    }
    before.0 + (to_before / total) * (after.0 - before.0)
}

/// Absorbe chaque passage plus court que `min_keep_km` dans son **voisin le plus
/// long**, jusqu'à stabilité.
///
/// Un passage de quelques centaines de mètres ne correspond qu'à un artefact
/// (frontière OSM imprécise, micro-chevauchement) et pollue la lecture. La
/// longueur totale est conservée : la surface absorbée est réattribuée au
/// voisin, dont les bornes sont étendues.
fn merge_short_segments(passages: &mut Vec<DraftPassage>, min_keep_km: f64) {
    let mut guard = 0;
    while passages.len() > 1 && guard < 100 {
        guard += 1;
        let Some(index) = passages.iter().position(|p| p.length_km() < min_keep_km) else {
            break;
        };
        let target = if index > 0 && index + 1 < passages.len() {
            if passages[index - 1].length_km() >= passages[index + 1].length_km() {
                index - 1
            } else {
                index + 1
            }
        } else if index > 0 {
            index - 1
        } else {
            index + 1
        };

        let absorbed = passages.remove(index);
        // L'index du voisin se décale si le passage absorbé le précède.
        let target = if target > index { target - 1 } else { target };
        if target < index {
            passages[target].exit_km = absorbed.exit_km;
        } else {
            passages[target].entry_km = absorbed.entry_km;
        }
    }
}

/// Recolle les passages adjacents d'une même commune (trou de timeline comblé
/// d'un seul côté, ou commune quittée puis rejointe sans autre commune entre).
fn merge_contiguous(passages: &mut Vec<DraftPassage>) {
    let mut guard = 0;
    while guard < 100 {
        guard += 1;
        let Some(index) = (1..passages.len())
            .find(|i| passages[*i].commune == passages[*i - 1].commune)
        else {
            break;
        };
        let merged = passages.remove(index);
        passages[index - 1].exit_km = merged.exit_km;
    }
}

/// Médiane d'une liste de valeurs (0 pour une liste vide).
fn median(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        values[middle]
    } else {
        (values[middle - 1] + values[middle]) / 2.0
    }
}

/// Numérote les passages : numéro chronologique et total par commune.
fn number(drafts: Vec<DraftPassage>, candidates: &[Candidate]) -> Vec<CommunePassage> {
    let mut totals: HashMap<usize, u32> = HashMap::new();
    for draft in &drafts {
        *totals.entry(draft.commune).or_insert(0) += 1;
    }

    let mut seen: HashMap<usize, u32> = HashMap::new();
    drafts
        .into_iter()
        .map(|draft| {
            let passage = seen.entry(draft.commune).or_insert(0);
            *passage += 1;
            // Bornes arrondies au mètre : la longueur est la différence des
            // bornes arrondies, l'invariant « longueur = sortie − entrée » tient
            // donc exactement dans le fichier écrit.
            let km_entree = round3(draft.entry_km);
            let km_sortie = round3(draft.exit_km);
            CommunePassage {
                commune: candidates[draft.commune].name.clone(),
                passage: *passage,
                nb_passages_total: totals[&draft.commune],
                km_entree,
                km_sortie,
                longueur_km: km_sortie - km_entree,
            }
        })
        .collect()
}

/// Arrondi au mètre (trois décimales de kilomètre).
fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}
