//! Détection des passages multiples d'une trace.
//!
//! Le pipeline de la spécification (« Multi-Sens » v5) s'ordonne ainsi :
//! rééchantillonnage à pas quasi constant → appariement des points superposés et
//! chaînage en runs → assemblage en segments (phases A/B/C itérées, frontières
//! aller/retour préservées) → qualification du sens de chaque passage.
//!
//! Les deux premières étapes sont en place : lecture du GPX, géométrie métrique
//! (projection locale et dédoublonnage) et rééchantillonnage. La détection
//! proprement dite produit aujourd'hui une liste de passages vide.

use std::path::Path;

use super::file;
use super::projection;
use super::resample;
use super::types::{MultirideArchive, MultirideParams, MultiridePassage, MultiridePoint};

/// Extrait les points bruts d'un GPX : suite ordonnée des `<trkpt>`, tous
/// segments de trace concaténés dans l'ordre du document.
///
/// C'est **l'espace d'index du contrat** : les numéros de points portés par le
/// fichier de description se rapportent à cette suite (cf.
/// `resample::raw_point_number`).
fn load_points(gpx_path: &Path) -> Result<Vec<MultiridePoint>, String> {
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
/// État de la sous-étape en cours : la lecture du GPX, la géométrie métrique
/// (dédoublonnage à 5 cm) et le rééchantillonnage à pas quasi constant sont en
/// place, tout comme le contrat de fichier et le verrou d'édition caméra. La
/// détection elle-même — appariement des points superposés, assemblage en
/// segments, qualification du sens — arrive avec les sous-étapes suivantes et
/// produit ici une liste de passages **vide**.
pub fn detect(
    trace_id: &str,
    source: &str,
    gpx_path: &Path,
    params: MultirideParams,
) -> Result<MultirideArchive, String> {
    let points = load_points(gpx_path)?;
    let geom = projection::build_geometry(&points)?;
    let sampling = resample::resample(&geom, params.pas_echantillonnage_m)?;

    let passages: Vec<MultiridePassage> = Vec::new();

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
