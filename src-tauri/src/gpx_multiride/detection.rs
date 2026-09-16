//! Détection des passages multiples d'une trace.
//!
//! Le pipeline de la spécification (« Multi-Sens » v5) s'ordonne ainsi :
//! rééchantillonnage à pas quasi constant → appariement des points
//! superposés et chaînage en runs → assemblage en segments (phases A/B/C
//! itérées, frontières aller/retour préservées) → qualification du sens de
//! chaque passage.
//!
//! Les étapes d'entrée (lecture du GPX, métrique de la trace) sont posées ici ;
//! elles ne dépendent pas de la détection proprement dite, qui produit
//! aujourd'hui une liste de passages vide.

use std::path::Path;

use crate::import_gpx::haversine;

use super::file;
use super::types::{MultirideArchive, MultirideParams, MultiridePassage, MultiridePoint};

/// Extrait les points bruts d'un GPX : suite ordonnée des `<trkpt>`, tous
/// segments de trace concaténés dans l'ordre du document.
///
/// Les points sans coordonnées exploitables ne sont pas filtrés ici : la
/// métrique et le dédoublonnage relèvent de la construction de la géométrie.
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

/// Longueur totale de la trace (km), par somme des distances orthodromiques
/// entre points consécutifs.
///
/// La spécification mesure la trace **brute** (avant rééchantillonnage et
/// consolidation) : c'est cette longueur qui figure dans le bloc `trace` du
/// fichier de description.
fn total_length_km(points: &[MultiridePoint]) -> f64 {
    let meters: f64 = points
        .windows(2)
        .map(|pair| haversine(pair[0].lat, pair[0].lon, pair[1].lat, pair[1].lon))
        .sum();
    meters / 1000.0
}

/// Détecte les passages multiples d'une trace et retourne l'état complet, prêt
/// à être écrit.
///
/// Sous-étape É1 : le socle. La lecture du GPX, la métrique de la trace, le
/// contrat de fichier et le verrou d'édition caméra sont en place ; la
/// détection elle-même (rééchantillonnage, appariement, assemblage,
/// qualification du sens) arrive avec les sous-étapes suivantes et produit ici
/// une liste de passages **vide**.
pub fn detect(
    trace_id: &str,
    source: &str,
    gpx_path: &Path,
    params: MultirideParams,
) -> Result<MultirideArchive, String> {
    let points = load_points(gpx_path)?;
    if points.len() < 2 {
        return Err("Trace dégénérée : moins de 2 points.".to_string());
    }

    let passages: Vec<MultiridePassage> = Vec::new();

    Ok(file::build_archive(
        trace_id,
        source,
        params,
        points.len(),
        total_length_km(&points),
        false,
        passages,
    ))
}
