//! Détection des passages multiples d'une trace.
//!
//! Le pipeline de la spécification (« Multi-Sens » v5) s'ordonne ainsi :
//! rééchantillonnage à pas quasi constant → appariement des points superposés et
//! chaînage en runs → assemblage en segments (phases A/B/C itérées, frontières
//! aller/retour préservées) → qualification du sens de chaque passage.
//!
//! Ce module est l'**orchestrateur** : il lit le GPX, enchaîne les étapes et
//! projette le résultat des algorithmes sur le contrat de fichier
//! (`MultirideArchive` et ses `MultiridePassage`).

use std::path::Path;

use super::projection;
use super::resample;
use super::runs;
use super::segments;
use super::types::{
    MultirideArchive, MultirideLatLon, MultirideParams, MultiridePassage, MultiridePoint,
    MultirideSens,
};
use super::file;

/// Extrait les points bruts d'un GPX : suite ordonnée des `<trkpt>`, tous
/// segments de trace concaténés dans l'ordre du document.
///
/// C'est **l'espace d'index du contrat** : les numéros de points portés par le
/// fichier de description se rapportent à cette suite (cf.
/// `resample::raw_point_number`). Les tests de bout en bout s'en servent pour
/// reconstruire les étapes intermédiaires du pipeline et vérifier la détection
/// indépendamment des valeurs qu'elle produit.
pub(crate) fn load_points(gpx_path: &Path) -> Result<Vec<MultiridePoint>, String> {
    let file = std::fs::File::open(gpx_path)
        .map_err(|e| format!("Ouverture du fichier GPX : {}", e))?;
    let gpx = gpx::read(std::io::BufReader::new(file))
        .map_err(|e| format!("Fichier GPX invalide : {}", e))?;

    let mut points = Vec::new();
    for track in &gpx.tracks {
        for segment in &track.segments {
            for wp in &segment.points {
                let pt = wp.point();
                points.push(MultiridePoint {
                    lat: pt.y(),
                    lon: pt.x(),
                });
            }
        }
    }
    Ok(points)
}

/// Détecte les passages multiples d'une trace et retourne l'état complet, prêt
/// à être écrit.
///
/// Enchaîne les étapes du pipeline puis projette les segments sur le contrat :
/// chaque emprunt devient un `MultiridePassage`, numéroté dans son segment et
/// dans l'ensemble, avec son sens, ses bornes kilométriques et ses numéros de
/// points du GPX d'origine.
pub fn detect(
    trace_id: &str,
    source: &str,
    gpx_path: &Path,
    params: MultirideParams,
) -> Result<MultirideArchive, String> {
    let points = load_points(gpx_path)?;
    let geom = projection::build_geometry(&points)?;
    let sampling = resample::resample(&geom, params.pas_echantillonnage_m)?;
    let runs = runs::find_runs(&sampling, params.tolerance_m, params.longueur_min_m)?;
    let found = segments::build_segments(
        &runs,
        &sampling,
        &geom,
        params.tolerance_m,
        params.fusion_references_m,
    );

    let mut passages: Vec<MultiridePassage> = Vec::new();
    for (segment_index, segment) in found.iter().enumerate() {
        for (passage_index, passage) in segment.passages.iter().enumerate() {
            passages.push(MultiridePassage {
                segment: segment_index + 1,
                passage: passage_index + 1,
                sens: match passage.rel {
                    0 => MultirideSens::Reference,
                    1 => MultirideSens::Aller,
                    _ => MultirideSens::Retour,
                },
                // L'utilisateur seul approuve un segment, le marque faux positif
                // ou le fusionne : une détection neuve n'en porte aucun, et n'a
                // donc rien à restaurer.
                faux_positif: false,
                point_entree: passage.pt0,
                point_sortie: passage.pt1,
                km_entree: passage.km0,
                km_sortie: passage.km1,
                longueur_km: passage.km1 - passage.km0,
                fusionne: false,
                valide: false,
                avant_fusion: None,
                entree: MultirideLatLon {
                    lat: sampling.lat[passage.s],
                    lon: sampling.lon[passage.s],
                },
                sortie: MultirideLatLon {
                    lat: sampling.lat[passage.e],
                    lon: sampling.lon[passage.e],
                },
            });
        }
    }

    // Diagnostic du pipeline : c'est ce que l'on suit pendant le portage de
    // l'algorithme, et ce que la relance de détection rend lisible.
    println!(
        "[multiride] pipeline trace={} points={} échantillons={} pas={:.2}m runs={} segments={} passages={}",
        trace_id,
        points.len(),
        sampling.n,
        sampling.step,
        runs.len(),
        found.len(),
        passages.len()
    );

    Ok(file::build_archive(
        trace_id,
        source,
        params,
        points.len(),
        // Longueur de la trace **analysée** : c'est la même métrique que les
        // bornes kilométriques des passages (`km_entree` / `km_sortie`), de
        // sorte que le consommateur du fichier lise les deux sur la même règle.
        geom.total / 1000.0,
        sampling.capped,
        passages,
    ))
}
